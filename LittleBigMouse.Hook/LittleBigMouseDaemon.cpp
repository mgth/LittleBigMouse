#include "LittleBigMouseDaemon.h"

#include "tinyxml2.h"

#include <shlobj.h>
#include <iostream>
#include <fstream>
#include <Shlwapi.h>
#pragma comment(lib,"shlwapi.lib")

#include "MouseEngine.h"
#include "SocketClient.h"
#include "XmlHelper.h"

LittleBigMouseDaemon::LittleBigMouseDaemon(MouseHooker& hook, RemoteServer& server, MouseEngine& engine):
	_hook(&hook),
	_remoteServer(&server),
	_engine(&engine)
{
	_remoteServer->SetDaemon(this);
	_hook->SetRemoteServer(_remoteServer);
}

void LittleBigMouseDaemon::Run() const
{
	_remoteServer->Start();

	LoadFromCurrentFile();

	_remoteServer->Join();
}

LittleBigMouseDaemon::~LittleBigMouseDaemon()
{
	_remoteServer->SetDaemon(nullptr);
}

void LittleBigMouseDaemon::ReceiveLoadMessage(tinyxml2::XMLElement* root) const
{
	if(!root) return;
	if(const auto zonesLayout = root->FirstChildElement("ZonesLayout"))
	{
		_hook->Stop();
		_hook->Engine()->Layout.Load(zonesLayout);
	}
}

std::string LittleBigMouseDaemon::GetStateMessage() const
{
	if(_hook)
		return "<DaemonMessage><State>Running</State></DaemonMessage>\n";
	else
		return "<DaemonMessage><State>Stopped</State></DaemonMessage>\n";
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

		if(strcmp(command, "LoadFromFile")==0)
			LoadFromFile(XmlHelper::GetString(root,"Payload"));

		else if(strcmp(command, "Run")==0)
			_hook->Start();

		else if(strcmp(command, "Stop")==0)
		{
			_hook->Stop();
		}
		else if(strcmp(command, "State")==0)
		{
			if(client)
				client->Send(GetStateMessage());
			else
				_remoteServer->Send(GetStateMessage());
		}

		else if(strcmp(command, "Quit")==0)
		{
			_hook->Stop();
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

void LittleBigMouseDaemon::ReceiveMessage(const std::string& m, RemoteClient* client = nullptr) const
{
	tinyxml2::XMLDocument doc;
	doc.Parse(m.c_str());

	ReceiveMessage(doc.RootElement(), client);
}

void LittleBigMouseDaemon::LoadFromCurrentFile() const
{
	LoadFromFile(TEXT("\\Mgth\\LittleBigMouse\\Current.xml"));
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
	if(path.empty())
	{
		LoadFromCurrentFile();
		return;
	}

    PWSTR szPath;

    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramData, 0, nullptr, &szPath)))
    {
	    std::ifstream startup;

        PathAppend(szPath, path.c_str());
	    startup.open(szPath, std::ios::in);

	    std::string buffer;
		std::string line;
		while(startup){
			std::getline(startup, line);
		    ReceiveMessage(line);
		}

	    startup.close();
    }

}
