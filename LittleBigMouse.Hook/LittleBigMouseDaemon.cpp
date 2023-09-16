#include "LittleBigMouseDaemon.h"
#include "tinyxml2.h"

LittleBigMouseDaemon::LittleBigMouseDaemon(RemoteServer& server, MouseEngine& engine):_engine(&engine),_remoteServer(&server)
{
	_remoteServer->SetDaemon(this);
	_engine->SetRemoteServer(_remoteServer);
}

void LittleBigMouseDaemon::Run() const
{
	_remoteServer->Start();
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
		_engine->Stop();
		_engine->Layout.Load(zonesLayout);
	}
}

void LittleBigMouseDaemon::ReceiveCommandMessage(tinyxml2::XMLElement* root) const
{
	if(!root) return;

	auto commandAttribut = root->FindAttribute("Command");
	if(commandAttribut)
	{
		const auto command = commandAttribut->Value();
		if(strcmp(command, "Load")==0)
			ReceiveLoadMessage(root->FirstChildElement("Payload"));

		else if(strcmp(command, "Run")==0)
			_engine->Start();

		else if(strcmp(command, "Stop")==0)
			_engine->Stop();

		else if(strcmp(command, "Quit")==0)
		{
			_engine->Stop();
			// TODO : quit exe
		}
	}
}
void LittleBigMouseDaemon::ReceiveMessage(tinyxml2::XMLElement* root) const
{
	if(!root) return;

	if(strcmp(root->Name(), "CommandMessage") ==0 ) 
		ReceiveCommandMessage(root);

	else if(strcmp(root->Name(), "Messages") ==0 )
	{
		auto node = root->FirstChildElement();
		while(node)
		{
			ReceiveMessage(node);
			node = node->NextSiblingElement();
		}
	}
}

void LittleBigMouseDaemon::ReceiveMessage(const std::string& m) const
{
	tinyxml2::XMLDocument doc;
	doc.Parse(m.c_str());

	ReceiveMessage(doc.RootElement());
}

