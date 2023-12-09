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
#include "str.h"

void LittleBigMouseDaemon::Send() const
{
	SendState(nullptr);
}

LittleBigMouseDaemon::LittleBigMouseDaemon(RemoteServer* server, MouseEngine* engine, Hooker* hook):
	_remoteServer(server),
	_engine(engine),
	_hook(hook)
{
}

void LittleBigMouseDaemon::Connect()
{
	if(_hook)
	{
		if(_engine)
			_hook->OnMouseMove.connect<&MouseEngine::OnMouseMove>(_engine);

		_hook->OnHooked.connect<&LittleBigMouseDaemon::Send>(this);
		_hook->OnUnhooked.connect<&LittleBigMouseDaemon::Send>(this);

		_hook->OnDisplayChanged.connect<&LittleBigMouseDaemon::DisplayChanged>(this);
		_hook->OnDesktopChanged.connect<&LittleBigMouseDaemon::DesktopChanged>(this);
		_hook->OnFocusChanged.connect<&LittleBigMouseDaemon::FocusChanged>(this);
	}
	if(_remoteServer)
	{
		_remoteServer->OnMessage.connect<&LittleBigMouseDaemon::ReceiveClientMessage>(this);
	}
}

void LittleBigMouseDaemon::Run(const std::string& path) 
{
// load excluded list
	LoadExcluded("\\Mgth\\LittleBigMouse\\Excluded.txt");

// load layout from file
	if(!path.empty())
		LoadFromFile(path);

	Connect();

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
		_hook->OnHooked.disconnect<&LittleBigMouseDaemon::Send>(this);
		_hook->OnUnhooked.disconnect<&LittleBigMouseDaemon::Send>(this);

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

void LittleBigMouseDaemon::ReceiveCommandMessage(tinyxml2::XMLElement* root, RemoteClient* client) 
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
				if(!_paused)
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
			_paused = false;
		}

		else if(strcmp(command, "State")==0)
			SendState(client);

		else if(strcmp(command, "Quit")==0)
		{
			if(_hook && _hook->Hooked())
			{
				_hook->SetPriority(Normal);
				_hook->Unhook();
			}
			_paused = false;

			if(_remoteServer)
				_remoteServer->Stop();
		}
	}
}

void LittleBigMouseDaemon::ReceiveMessage(tinyxml2::XMLElement* root, RemoteClient* client) 
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
	{
		_remoteServer->Send("<DaemonMessage><Event>Running</Event></DaemonMessage>\n",client);
	}
	else
	{
		if(_paused)
			_remoteServer->Send("<DaemonMessage><Event>Paused</Event></DaemonMessage>\n",client);
		else
			_remoteServer->Send("<DaemonMessage><Event>Stopped</Event></DaemonMessage>\n",client);
	}
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

bool LittleBigMouseDaemon::Excluded(const std::string& path) const
{

	for (auto &line : _excluded) 
	{  
		if (line.length() > 1 && path.find(line) < path.length())
	    {
			#if defined(_DEBUG)
			std::cout << "<daemon:excluded found> : " << line << "\n";
			#endif
			return true;
		}
	}
	return false;
}

// Window focus has changed
void LittleBigMouseDaemon::FocusChanged(const std::string& path) 
{
	if(Excluded(path))
	{
		#if defined(_DEBUG)
		std::cout << "<daemon:excluded>" << "\n";
		#endif
		if(_paused) return;

		if(_hook && _hook->Hooked())
		{
			_hook->Unhook();
			_paused = true;
			#if defined(_DEBUG)
			std::cout << "<daemon:paused>" << "\n";
			#endif
		}
	}
    else
    {
		std::cout << "<daemon:included>" << "\n";
		if(!_paused) return;

		if(_hook && !_hook->Hooked())
		{
			_hook->Hook();
		}
		_paused = false;

		#if defined(_DEBUG)
		std::cout << "<daemon:wakeup>" << "\n";
		#endif
	}

	//if(_hook && _hook->Hooked())
	//	_hook->Stop();
	_remoteServer->Send("<DaemonMessage><Event>FocusChanged</Event><Payload>"+path+"</Payload></DaemonMessage>\n",nullptr);
}

void LittleBigMouseDaemon::ReceiveClientMessage(const std::string& message, RemoteClient* client)
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

void LittleBigMouseDaemon::LoadExcluded(const std::string& path) 
{
	auto wPath = to_wstring(path);

    PWSTR szPath;

    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramData, 0, nullptr, &szPath)))
    {
	    std::ifstream file;

        PathAppend(szPath, wPath.c_str());
	    file.open(szPath, std::ios::in);

	    std::string buffer;
		std::string line;
		while(file)
		{
			std::getline(file, line);

			if(line.empty()) continue;
			if(line[0] == ':') continue;

		    _excluded.push_back(line);

			#if defined(_DEBUG)
			std::cout << "Excluded : " << line << "\n";
			#endif
		}

	    file.close();
    }
}

void LittleBigMouseDaemon::LoadFromFile(const std::string& path)
{
    PWSTR szPath = nullptr;
	long result = 0;

	try{
		result = SHGetKnownFolderPath(FOLDERID_ProgramData, 0, nullptr, &szPath);
	}
	catch (...)
	{
		#if defined(_DEBUG)
		std::cout << "Failed to get ProgramData folder\n";
		#endif
		return;
	}

    if (result == S_OK)
    {
	    std::ifstream file;

        PathAppend(szPath, to_wstring(path).c_str());
	    file.open(szPath, std::ios::in);

	    std::string buffer;
		std::string line;
		while(!file.eof()){
			std::getline(file, line);
		    ReceiveClientMessage(line,nullptr);
		}

	    file.close();
    }
	else
	{
		#if defined(_DEBUG)
		std::cout << "Failed to load layout from file : " << path << "\n";
		#endif
	}

}
