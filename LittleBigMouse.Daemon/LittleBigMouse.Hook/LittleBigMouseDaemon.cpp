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

LittleBigMouseDaemon::LittleBigMouseDaemon(RemoteServer* server, MouseEngine* engine, Hooker* hook):
	_remoteServer(server),
	_engine(engine),
	_hook(hook)
{
	if(_hook)
	{
		if(_engine)
			_hook->OnMouseMove.connect<&MouseEngine::OnMouseMove>(_engine);

		_hook->OnMessage.connect<&LittleBigMouseDaemon::Send>(this);

		_hook->OnDisplayChanged.connect<&LittleBigMouseDaemon::DisplayChanged>(this);
		_hook->OnDesktopChanged.connect<&LittleBigMouseDaemon::DesktopChanged>(this);
		_hook->OnFocusChanged.connect<&LittleBigMouseDaemon::FocusChanged>(this);
	}
	if(_remoteServer)
	{
		_remoteServer->OnMessage.connect<&LittleBigMouseDaemon::ReceiveClientMessage>(this);
	}
}

void LittleBigMouseDaemon::Run(const std::string& path) const
{
// load layout from file
	if(!path.empty())
		LoadFromFile(path);

//start remote server
	if(_remoteServer)
		_remoteServer->Start();

// pump messages
	_hook->Start();

// wait remote server to stop
	if(_remoteServer)
		_remoteServer->Join();

// wait for mouse hook to stop
	_hook->Stop();
}

LittleBigMouseDaemon::~LittleBigMouseDaemon()
{
	if(_hook)
	{
		_hook->OnMouseMove.disconnect<&MouseEngine::OnMouseMove>(_engine);
		_hook->OnMessage.disconnect<&LittleBigMouseDaemon::Send>(this);

		_hook->OnDisplayChanged.disconnect<&LittleBigMouseDaemon::DisplayChanged>(this);
		_hook->OnDesktopChanged.disconnect<&LittleBigMouseDaemon::DesktopChanged>(this);
		_hook->OnFocusChanged.disconnect<&LittleBigMouseDaemon::FocusChanged>(this);

	}
	if(_remoteServer)
		_remoteServer->OnMessage.disconnect<&LittleBigMouseDaemon::ReceiveClientMessage>(this);
}

void LittleBigMouseDaemon::ReceiveLoadMessage(tinyxml2::XMLElement* root) const
{
	if(!root) return;
	if(const auto zonesLayout = root->FirstChildElement("ZonesLayout"))
	{

		if(_hook && _hook->Hooked())
			_hook->Unhook();

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

		#if defined(_DEBUG)
		std::cout << command << "\n";
		#endif

		if(strcmp(command, "Load")==0)
			ReceiveLoadMessage(root->FirstChildElement("Payload"));

		else if(strcmp(command, "LoadFromFile")==0)
			LoadFromFile(XmlHelper::GetString(root,"Payload"));

		else if(strcmp(command, "Run")==0)
		{
			if(_hook && !_hook->Hooked())
			{
				_hook->SetPriority(_engine->Layout.Priority);
				_hook->Hook();
			}
		}

		else if(strcmp(command, "Stop")==0)
		{
			if(_hook && _hook->Hooked())
			{
				_hook->SetPriority(Normal);
				_hook->Unhook();
			}
		}

		else if(strcmp(command, "State")==0)
			SendState(client);

		else if(strcmp(command, "Quit")==0)
		{
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
		_remoteServer->Send("<DaemonMessage><Event>Running</Event></DaemonMessage>\n",client);
	else
		_remoteServer->Send("<DaemonMessage><Event>Stopped</Event></DaemonMessage>\n",client);
}

// Display configuration has changed.
void LittleBigMouseDaemon::DisplayChanged() const
{
	//When display changed, we need to recompute zones, here we just stop the hook and inform ui to reload layout
	if(_hook && _hook->Hooked())
		_hook->Unhook();

	_remoteServer->Send("<DaemonMessage><Event>DisplayChanged</Event></DaemonMessage>\n",nullptr);
}

// Sytem switches to/from UAC desktop
void LittleBigMouseDaemon::DesktopChanged() const
{

	_remoteServer->Send("<DaemonMessage><Event>DesktopChanged</Event></DaemonMessage>\n",nullptr);
}

// Window focus has changed
void LittleBigMouseDaemon::FocusChanged(const std::wstring& wpath) const
{
	std::string path( wpath.begin(), wpath.end() );
	//if(_hook && _hook->Hooked())
	//	_hook->Stop();
	_remoteServer->Send("<DaemonMessage><Event>FocusChanged</Event><Payload>"+path+"</Payload></DaemonMessage>\n",nullptr);
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
