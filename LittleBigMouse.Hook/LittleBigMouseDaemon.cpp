#include "LittleBigMouseDaemon.h"
#include "tinyxml2.h"

void LittleBigMouseDaemon::ReceiveMessage(std::string m)
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
					_engine.Stop();
					_engine.Layout.Load(zonesLayout);
				}
			}
		}

		if(command=="Run")
		{
			_engine.Start();
		}

		if(command=="Stop")
		{
			_engine.Stop();
		}

		if(command=="quit")
		{
			_engine.Stop();
		}
	}

}

