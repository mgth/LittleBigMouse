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

void LittleBigMouseDaemon::ReceiveMessage(const std::string& m) const
{
	tinyxml2::XMLDocument doc;
	doc.Parse(m.c_str());

	auto root = doc.RootElement();
	if(!root) return;

	auto name = std::string(root->Name());

	auto c = root->Name();
	auto e = strcmp(root->Name(), "DaemonMessage");

	if(strcmp(root->Name(), "DaemonMessage") !=0 ) return;

	auto commandAttribut = root->FindAttribute("Command");
	if(commandAttribut)
	{
		auto command = std::string(commandAttribut->Value());
		if(command == "Load")
		{
			auto payloadElement = root->FirstChildElement("Payload");
			if(payloadElement)
			{
				auto zonesLayout = payloadElement->FirstChildElement("ZonesLayout");
				if(zonesLayout)
				{
					_engine->Stop();
					_engine->Layout.Load(zonesLayout);
				}
			}
		}

		if(command=="Run")
		{
			_engine->Start();
		}

		if(command=="Stop")
		{
			_engine->Stop();
		}

		if(command=="Quit")
		{
			_engine->Stop();
		}
	}

}

