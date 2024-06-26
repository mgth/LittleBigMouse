#include "LittleBigMouseDaemon.h"

#include <shlobj.h>
#include <fstream>
#include <Shlwapi.h>
#pragma comment(lib,"shlwapi.lib")

#include "SignalSlot.h"

#include "Engine/MouseEngine.h"
#include "Hook/Hooker.h"
#include "Remote/RemoteServer.h"
#include "Remote/RemoteClient.h"
#include "Xml/tinyxml2.h"
#include "Xml/XmlHelper.h"
#include "Strings/str.h"

void LittleBigMouseDaemon::Unhooked() const
{
	SendState(nullptr);
}

void LittleBigMouseDaemon::Hooked() const
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
#ifdef SIGS_SIGNAL_SLOT_H
	if(_hook)
	{
		if(_engine)
		{
			#if defined(_DEBUG_)
			_engine->DebugUnhook.connect(_hook, &Hooker::DoUnhook);
			#endif

			_hook->OnMouseMove.connect(_engine, &MouseEngine::OnMouseMove);
		}

		_hook->OnHooked.connect(this, &LittleBigMouseDaemon::Hooked);
		_hook->OnUnhooked.connect(this, &LittleBigMouseDaemon::Unhooked);

		_hook->OnDisplayChanged.connect(this, &LittleBigMouseDaemon::DisplayChanged);
		_hook->OnSettingChanged.connect(this, &LittleBigMouseDaemon::SettingChanged);
		_hook->OnDesktopChanged.connect(this, &LittleBigMouseDaemon::DesktopChanged);
		_hook->OnFocusChanged.connect(this, &LittleBigMouseDaemon::FocusChanged);
	}
	if(_remoteServer)
	{
		_remoteServer->OnMessage.connect(this, &LittleBigMouseDaemon::ReceiveClientMessage);
	}

#else
	if(_hook)
	{
		if(_engine)
		{
			#if defined(_DEBUG_)
			_engine->DebugUnhook.connect<&Hooker::DoUnhook>(_hook);
			#endif

			_hook->OnMouseMove.connect<&MouseEngine::OnMouseMove>(_engine);
		}

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
#endif
}


void LittleBigMouseDaemon::Run(const std::string& path) 
{
	LOG_TRACE("Daemon started");
	// connect to events
	Connect();
	LOG_TRACE("Connected");

	//start remote server
	if(_remoteServer)
	{
		_remoteServer->Start();
		_remoteServer->WaitForReady(10);
		if(!_remoteServer->IsRunning())
		{
			LOG_ERROR("Failed to start server");
			return;
		}
	}
	LOG_TRACE("RemoteServer");

	// load layout from file
	if(!path.empty())
		LoadFromFile(path);
	
	// pump messages
	if(_hook)
		_hook->Loop();

	// wait remote server to stop
	if(_remoteServer)
	{
		_remoteServer->Stop();
		_remoteServer->Join();
	}

	// disconnect from events
	Disconnect();
}

void LittleBigMouseDaemon::Disconnect()
{
#if defined(SIGS_SIGNAL_SLOT_H)
	if(_hook)
	{
		_hook->OnMouseMove.disconnect();
		_hook->OnHooked.disconnect();
		_hook->OnUnhooked.disconnect();

		_hook->OnDisplayChanged.disconnect();
		_hook->OnDesktopChanged.disconnect();
		_hook->OnFocusChanged.disconnect();

	}
	if(_remoteServer)
		_remoteServer->OnMessage.disconnect();
#else
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
#endif
}

void LittleBigMouseDaemon::ReceiveListenMessage(RemoteClient* client) const
{
	if (!client) return;
	client->Listen();
	SendState(client);
}

void LittleBigMouseDaemon::ReceiveLoadMessage(tinyxml2::XMLElement* root) const
{
	if(!root) return;
	if(const auto zonesLayout = root->FirstChildElement("ZonesLayout"))
	{

		if(_hook && _hook->Hooked())
			_hook->Unhook();

		if(_engine)
		{
			_engine->Layout.Load(zonesLayout);
			if(_hook)
			{
				_hook->SetPriority(_engine->Layout.Priority);
				_hook->SetPriorityUnhooked(_engine->Layout.PriorityUnhooked);
			}
		}
	}
}

void LittleBigMouseDaemon::ReceiveCommandMessage(tinyxml2::XMLElement* root, RemoteClient* client) 
{
	if(!root) return;

	if(const auto commandAttribute = root->FindAttribute("Command"))
	{
		const auto command = commandAttribute->Value();

		LOG_TRACE("Command received : " << command);


		if(strcmp(command, "Listen")==0)
			ReceiveListenMessage(client);

		else if(strcmp(command, "Load")==0)
			ReceiveLoadMessage(root->FirstChildElement("Payload"));

		else if(strcmp(command, "LoadFromFile")==0)
			LoadFromFile(XmlHelper::GetString(root,"Payload"));

		else if(strcmp(command, "Run")==0)
		{
			if(_hook && !_hook->Hooked())
			{
				LoadExcluded();
				if(!_paused)
					_hook->Hook();
			}
		}

		else if(strcmp(command, "Stop")==0)
		{
			if(_hook && _hook->Hooked())
			{
				_hook->Unhook();
			}
			_paused = false;
		}

		else if(strcmp(command, "State")==0)
			SendState(client);

		else if(strcmp(command, "Quit")==0)
		{
			if(_hook)
			{
				_hook->Quit();
			}
			_paused = false;
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

void LittleBigMouseDaemon::SettingChanged() const
{
	//When display changed, we need to recompute zones, here we just stop the hook and inform ui to reload layout
	if(_hook && _hook->Hooked())
		_hook->Unhook();

	_remoteServer->Send("<DaemonMessage><Event>SettingChanged</Event></DaemonMessage>\n",nullptr);
}

// Sytem switches to/from UAC desktop
void LittleBigMouseDaemon::DesktopChanged() const
{
	_remoteServer->Send("<DaemonMessage><Event>DesktopChanged</Event></DaemonMessage>\n",nullptr);
}

bool LittleBigMouseDaemon::Excluded(const std::string& path) const
{
	if(path.empty()) return false;
	for (auto &line : _excluded) 
	{  
		if (line.length() > 1 && path.find(line) < path.length())
	    {
			LOG_TRACE("<daemon:excluded found> : " << line);
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
		LOG_TRACE("<daemon:excluded>");
		if(!_paused)
		{
			if(_hook && _hook->Hooked())
			{
				_hook->Unhook();
				_paused = true;
				LOG_TRACE("<daemon:paused>");
			}
		}
	}
    else
    {
		if(_paused)
		{
			if(_hook && !_hook->Hooked())
			{
				_hook->Hook();
			}
			_paused = false;

			LOG_TRACE("<daemon:wakeup>");
		}
	}

	if(_remoteServer)
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

	LPWSTR progDataPath = nullptr;
	_excluded.clear();

    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramData, 0, nullptr, &progDataPath)))
    {
	    std::ifstream file;

		TCHAR szPath[MAX_PATH] = { '\0' };
		PathCombine(szPath, progDataPath, ToWString(path).data());
		CoTaskMemFree(progDataPath);

		LOG_TRACE("Load Excluded : " << ToString(szPath));

	    file.open(szPath, std::ios::in);

		std::string line;
		while(std::getline(file, line))
		{

			if(line.empty()) continue;
			if(line[0] == ':') continue;

			LOG_TRACE("Excluded : " << line);
		    _excluded.push_back(line);

		}
	    file.close();
    }
	else
	{
		LOG_DEBUG("Failed to load excluded");
	}


}

void LittleBigMouseDaemon::LoadExcluded()
{
	LoadExcluded(R"(.\Mgth\LittleBigMouse\Excluded.txt)");
}

void LittleBigMouseDaemon::LoadFromFile(const std::string& path)
{
	LPWSTR progDataPath = nullptr;

    if (SUCCEEDED(SHGetKnownFolderPath(FOLDERID_ProgramData, 0, nullptr, &progDataPath)))
    {
	    std::ifstream file;

		TCHAR szPath[MAX_PATH] = { '\0' };
		PathCombine(szPath, progDataPath, ToWString(path).data());
		CoTaskMemFree(progDataPath);

	    file.open(szPath, std::ios::in);

		std::string line;
		while(file)
		{
			std::getline(file, line);
		    ReceiveClientMessage(line,nullptr);
		}

	    file.close();
    }

}
