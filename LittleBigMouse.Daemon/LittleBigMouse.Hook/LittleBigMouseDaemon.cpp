#include "LittleBigMouseDaemon.h"

#include "tinyxml2.h"

#include <shlobj.h>
#include <iostream>
#include <fstream>
#include <Shlwapi.h>
#pragma comment(lib,"shlwapi.lib")

#include "ClientMessage.h"
#include "MouseEngine.h"
#include "Hooker.h"
#include "RemoteClient.h"
#include "XmlHelper.h"

void LittleBigMouseDaemon::Send(const std::string& string) const
{
	_remoteServer->Send(string,nullptr);
}

LittleBigMouseDaemon::LittleBigMouseDaemon(Hooker* hook, RemoteServer* server, MouseEngine* engine):
	_hook(hook),
	_engine(engine),
	_remoteServer(server)
{
	if(_hook)
	{
		_hook->OnMouseMove.connect<&MouseEngine::OnMouseMove>(_engine);
		_hook->OnMessage.connect<&LittleBigMouseDaemon::Send>(this);
	}
	if(_remoteServer)
	{
		_remoteServer->OnMessage.connect<&LittleBigMouseDaemon::ReceiveClientMessage>(this);
	}
}

void LittleBigMouseDaemon::Run(const std::string& path) const
{
	//start remote server
	if(_remoteServer)
		_remoteServer->Start();

	// load layout from file
	if(!path.empty())
		LoadFromFile(path);

	// wait remote server to stop
	if(_remoteServer)
		_remoteServer->Join();

	// wait for mouse hook to stop
	_hook->DoStop();
	if(_hook)
		_hook->Join();
}

LittleBigMouseDaemon::~LittleBigMouseDaemon()
{
	if(_hook)
	{
		_hook->OnMouseMove.disconnect<&MouseEngine::OnMouseMove>(_engine);
		_hook->OnMessage.disconnect<&LittleBigMouseDaemon::Send>(this);
	}
	if(_remoteServer)
		_remoteServer->OnMessage.disconnect<&LittleBigMouseDaemon::ReceiveClientMessage>(this);
}

void LittleBigMouseDaemon::ReceiveLoadMessage(tinyxml2::XMLElement* root) const
{
	if(!root) return;
	if(const auto zonesLayout = root->FirstChildElement("ZonesLayout"))
	{
		if(_hook)
			_hook->Stop();

		if(_engine)
			_engine->Layout.Load(zonesLayout);
	}
}

void LittleBigMouseDaemon::ReceiveCommandMessage(tinyxml2::XMLElement* root, RemoteClient* client) const
{
	if(!root) return;

	if(const auto commandAttribute = root->FindAttribute("Command"))
	{
		const auto command = commandAttribute->Value();

		std::cout << command << "\n";

		if(strcmp(command, "Load")==0)
			ReceiveLoadMessage(root->FirstChildElement("Payload"));

		else if(strcmp(command, "LoadFromFile")==0)
			LoadFromFile(XmlHelper::GetString(root,"Payload"));

		else if(strcmp(command, "Run")==0)
		{
			if(_hook && !_hook->Hooked())
				_hook->Start();
		}

		else if(strcmp(command, "Stop")==0)
		{
			if(_hook && _hook->Hooked())
				_hook->Stop();
		}

		else if(strcmp(command, "State")==0)
			SendState(client);

		else if(strcmp(command, "Quit")==0)
		{
			if(_hook && _hook->Hooked())
				_hook->Stop();
			if(_remoteServer)
				_remoteServer->Stop();


		}
	}
}

void LittleBigMouseDaemon::ReceiveMessage(tinyxml2::XMLElement* root, RemoteClient* client) const
{
	if(!root) return;

	if(strcmp(root->Name(), "CommandMessage") ==0 ) 
		ReceiveCommandMessage(root, client);

	else if(strcmp(root->Name(), "Messages") ==0 )
	{
		auto node = root->FirstChildElement();
		while(node)
		{
			ReceiveMessage(node, client);
			node = node->NextSiblingElement();
		}
	}
}

void LittleBigMouseDaemon::SendState(RemoteClient* client) const
{
	if(!_remoteServer) return;

	if(_hook && _hook->Hooked())
		_remoteServer->Send("<DaemonMessage><State>Running</State></DaemonMessage>\n",client);
	else
		_remoteServer->Send("<DaemonMessage><State>Stopped</State></DaemonMessage>\n",client);
}

void LittleBigMouseDaemon::ReceiveClientMessage(const std::string& message, RemoteClient* client) const
{
	if (message.empty())
	{
		SendState(client);
		return;
	}

	tinyxml2::XMLDocument doc;
	doc.Parse(message.c_str());

	ReceiveMessage(doc.RootElement(), client);
}

void LittleBigMouseDaemon::LoadFromFile(const std::string& path) const
{
	int convertResult = MultiByteToWideChar(CP_UTF8, 0, path.c_str(), path.length(), nullptr, 0); 
	if(convertResult>=0)
	{
		std::wstring wide;
		std::string s = path;
		s.resize(convertResult + 10);
		convertResult = MultiByteToWideChar(CP_UTF8, 0, s.c_str(), s.length(), &wide[0], wide.size());
		if(convertResult>=0)
			LoadFromFile(wide);
	}

}

void LittleBigMouseDaemon::LoadFromFile(const std::wstring& path) const
{
    PWSTR szPath;

    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramData, 0, nullptr, &szPath)))
    {
	    std::ifstream file;

        PathAppend(szPath, path.c_str());
	    file.open(szPath, std::ios::in);

	    std::string buffer;
		std::string line;
		while(file){
			std::getline(file, line);
		    ReceiveClientMessage(line,nullptr);
		}

	    file.close();
    }

}
