// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
#include "network.h"

// OpenZWave
#include <Defs.h>
#include <Driver.h>
#include <Group.h>
#include <Manager.h>
#include <Options.h>
#include <platform/Log.h>
#include <value_classes/ValueBool.h>
#include <value_classes/ValueStore.h>

#include <unistd.h>

using std::cout;
using std::endl;
using std::mutex;
using std::string;
using std::unique_lock;
using std::vector;
using std::unordered_map;

namespace ozw = OpenZWave;

Network::Network(string dev_name, bool verbose)
  : device_name(dev_name)
  , home_id(0)
  , done(false)
  , failed(false)
  , event_listener(nullptr)
  , event_listener_data(nullptr)
  , value_listener(nullptr)
  , value_listener_data(nullptr)
{
    ozw::Options::Create("./config/", "", "--SaveConfiguration=true --DumpTriggerLevel=0");
    //ozw::Options::Get()->Lock();
    //ozw::Options::Create("/usr/local/etc/openzwave/", "./var/cache/openzwave", "");
    if (verbose) {
        ozw::Options::Get()->AddOptionInt("SaveLogLevel", ozw::LogLevel_Detail);
        ozw::Options::Get()->AddOptionInt("QueueLogLevel", ozw::LogLevel_Debug);
        ozw::Options::Get()->AddOptionInt("DumpTrigger", ozw::LogLevel_Debug);
    } else {
        ozw::Options::Get()->AddOptionInt("SaveLogLevel", ozw::LogLevel_Error);
        ozw::Options::Get()->AddOptionInt("QueueLogLevel", ozw::LogLevel_Error);
        ozw::Options::Get()->AddOptionInt("DumpTrigger", ozw::LogLevel_Error);
    }
    ozw::Options::Get()->AddOptionInt("PollInterval", 500);
    ozw::Options::Get()->AddOptionBool("IntervalBetweenPolls", true);
    ozw::Options::Get()->AddOptionBool("ValidateValueChanges", true);
    ozw::Options::Get()->Lock();
    ozw::Manager::Create();
}

Network::~Network()
{
    ozw::Manager::Get()->RemoveDriver(device_name.c_str());
    ozw::Manager::Destroy();
    ozw::Options::Destroy();
}

bool
Network::init()
{
    ozw::Manager::Get()->AddWatcher(InitHandler, this);
    ozw::Manager::Get()->AddDriver(device_name.c_str());

    unique_lock<mutex> guard(lock);
    guard_done.wait(guard, [this](){return done;});

    ozw::Manager::Get()->RemoveWatcher(InitHandler, this);
    cout << endl;

    return !failed;
}

static void
Poke(char c)
{
    cout << c;
    cout.flush();
}

/* static */ void
Network::InitHandler(ozw::Notification const* notification, void* _context)
{
    auto network = static_cast<Network*>(_context);
    unique_lock<mutex> guard(network->lock);

    if (network->home_id)
        assert(network->home_id == notification->GetHomeId());

    switch (notification->GetType()) {
        case ozw::Notification::Type_ValueAdded:
        {
            Node& node = network->nodes[notification->GetNodeId()];
            node.values.push_back(notification->GetValueID());
            Poke('.');
            break;
        }

        case ozw::Notification::Type_NodeNew:
        case ozw::Notification::Type_NodeAdded:
        {
            uint8_t id = notification->GetNodeId();
            auto* m = ozw::Manager::Get();
            auto& hid = network->home_id;
            network->nodes[id] = Node(id);
            network->nodes[id].productName = m->GetNodeProductName(hid, id);
            network->nodes[id].productType = m->GetNodeProductType(hid, id);
            network->nodes[id].productId = m->GetNodeProductId(hid, id);
            network->nodes[id].manufacturerName = m->GetNodeManufacturerName(hid, id);
            network->nodes[id].manufacturerId = m->GetNodeManufacturerId(hid, id);
            Poke('@');
            break;
        }

        case ozw::Notification::Type_ValueRefreshed:
            Poke('r');
            break;

        case ozw::Notification::Type_ValueChanged:
            Poke('v');
            break;

        case ozw::Notification::Type_Group:
            Poke('g');
            break;

        case ozw::Notification::Type_NodeEvent:
            Poke('!');
            break;

        case ozw::Notification::Type_DriverReady:
            network->home_id = notification->GetHomeId();
            break;

        case ozw::Notification::Type_DriverFailed:
            Poke('F');
            network->failed = true;
        case ozw::Notification::Type_AwakeNodesQueried:
            Poke('X');
        case ozw::Notification::Type_AllNodesQueried:
            Poke('Y');
        case ozw::Notification::Type_AllNodesQueriedSomeDead:
            Poke('Z');
            network->done = true;
            network->guard_done.notify_all();
            break;

        case ozw::Notification::Type_NodeNaming:
            Poke('N');
            break;

        case ozw::Notification::Type_DriverReset:
        case ozw::Notification::Type_Notification:
        case ozw::Notification::Type_NodeProtocolInfo:
        case ozw::Notification::Type_NodeQueriesComplete:
        case ozw::Notification::Type_EssentialNodeQueriesComplete:
        case ozw::Notification::Type_DriverRemoved:
            break;

        case ozw::Notification::Type_ValueRemoved:
            assert(!"did not expect value removal");
            break;

        case ozw::Notification::Type_NodeRemoved:
            assert(!"did not expect node removal");
            break;

        case ozw::Notification::Type_PollingEnabled:
        case ozw::Notification::Type_PollingDisabled:
            assert(!"did not expect poll state!");
            break;

        case ozw::Notification::Type_NodeReset:
            assert(!"did not expect node reset");

        case ozw::Notification::Type_SceneEvent:
            assert(!"did not expect scene event");
        case ozw::Notification::Type_ControllerCommand:
            //assert(!"did not expect controller command");
            break;

        case ozw::Notification::Type_CreateButton:
        case ozw::Notification::Type_DeleteButton:
        case ozw::Notification::Type_ButtonOn:
        case ozw::Notification::Type_ButtonOff:
            assert(!"did not expect button presses");
    }
}

void
Network::begin_watching()
{
    ozw::Manager::Get()->AddWatcher(ListenEventsHandler, this);
}

void
Network::listen_events(ListenerEventCallbackType callback, void* data)
{
    assert(event_listener == nullptr);
    event_listener = callback;
    event_listener_data = data;
}

void
Network::listen_value_changes(ListenerValueCallbackType callback, void* data)
{
    assert(value_listener == nullptr);
    value_listener = callback;
    value_listener_data = data;
}

/* static */ void
Network::ListenEventsHandler(ozw::Notification const* notification, void* _context)
{
    auto network = static_cast<Network*>(_context);
    unique_lock<mutex> guard(network->lock);

    if (network->home_id)
        assert(network->home_id == notification->GetHomeId());

    switch (notification->GetType()) {
        case ozw::Notification::Type_NodeEvent:
        {
            uint8_t id = notification->GetNodeId();
            uint8_t event = notification->GetEvent();
            if (network->event_listener) {
                network->event_listener(id, event, network->event_listener_data);
            }
            break;
        }

        case ozw::Notification::Type_ValueChanged:
        {
            uint8_t id = notification->GetNodeId();
            ozw::ValueID val = notification->GetValueID();
            string label = ozw::Manager::Get()->GetValueLabel(val);
            string value;
            ozw::Manager::Get()->GetValueAsString(val, &value);
            if (network->value_listener) {
                network->value_listener(id, label, value,
                                       network->value_listener_data);
            }
            break;
        }

        case ozw::Notification::Type_Notification:
        {
            //auto error = (ozw::Notification::NotificationCode)notification->GetNotification();
            break;
        }

        case ozw::Notification::Type_ValueAdded:        Poke('.'); break;
        case ozw::Notification::Type_NodeNew:           Poke('N'); break;
        case ozw::Notification::Type_NodeAdded:         Poke('@'); break;
        case ozw::Notification::Type_ValueRefreshed:    Poke('r'); break;
        case ozw::Notification::Type_Group:             Poke('g'); break;
        case ozw::Notification::Type_DriverReady:       Poke('d'); break;
        case ozw::Notification::Type_DriverFailed:      Poke('F'); break;
        case ozw::Notification::Type_AwakeNodesQueried: Poke('X'); break;
        case ozw::Notification::Type_AllNodesQueried:   Poke('Y'); break;
        case ozw::Notification::Type_AllNodesQueriedSomeDead: Poke('Z'); break;
        case ozw::Notification::Type_NodeNaming:        Poke('a'); break;
        case ozw::Notification::Type_DriverReset:       Poke('D'); break;
        case ozw::Notification::Type_NodeProtocolInfo:  Poke('I'); break;
        case ozw::Notification::Type_NodeQueriesComplete: Poke('Q'); break;
        case ozw::Notification::Type_EssentialNodeQueriesComplete: Poke('E'); break;
        case ozw::Notification::Type_DriverRemoved:     Poke('R'); break;
        case ozw::Notification::Type_ValueRemoved:      Poke('V'); break;
        case ozw::Notification::Type_NodeRemoved:       Poke('v'); break;
        case ozw::Notification::Type_PollingEnabled:    Poke('P'); break;
        case ozw::Notification::Type_PollingDisabled:   Poke('p'); break;
        case ozw::Notification::Type_NodeReset:         Poke('r'); break;
        case ozw::Notification::Type_SceneEvent:        Poke('S'); break;
        case ozw::Notification::Type_ControllerCommand: Poke('C'); break;
        case ozw::Notification::Type_CreateButton:      Poke('B'); break;
        case ozw::Notification::Type_DeleteButton:      Poke('b'); break;
        case ozw::Notification::Type_ButtonOn:          Poke('O'); break;
        case ozw::Notification::Type_ButtonOff:         Poke('o'); break;
        default:                                        Poke('!'); break;
    }
}

void
Network::show(bool verbose)
{
    cout << "HomeID: " << home_id << endl;
    for (uint16_t i = 0; i < 256; ++i) {
        try {
            auto& node = nodes.at(i);
            if (verbose) {
                node.showBasicInfo();
                node.showValueGenre("Basic", ozw::ValueID::ValueGenre_Basic);
                node.showValueGenre("User", ozw::ValueID::ValueGenre_User);
                node.showValueGenre("Config", ozw::ValueID::ValueGenre_Config);
                node.showValueGenre("System", ozw::ValueID::ValueGenre_System);
            } else {
                cout << "\t" << +node.id
                     << " " << node.manufacturerName
                     << " " << node.productName
                     << endl;
            }
        } catch(std::out_of_range _) {}
    }
}

void
Network::Node::showBasicInfo()
{
    cout << "\tNode: " << +id << endl;
    cout << "\t\tProductName: " << productName << endl;
    cout << "\t\tProductType: " << productType << endl;
    cout << "\t\tProductId: " << productId << endl;
    cout << "\t\tManufacturerName: " << manufacturerName << endl;
    cout << "\t\tManufacturerId: " << manufacturerId << endl;
}

void
Network::Node::showValueGenre(const string& name, ozw::ValueID::ValueGenre genre)
{
    bool haveHeader = false;
    for (auto value : values) {
        if (value.GetGenre() == genre) {
            if (!haveHeader) {
                cout << "\t\t" << name << " Values:" << endl;
                haveHeader = true;
            }
            auto label = ozw::Manager::Get()->GetValueLabel(value);
            auto units = ozw::Manager::Get()->GetValueUnits(value);
            string v;
            ozw::Manager::Get()->GetValueAsString(value, &v);
            cout << "\t\t\t" << label << ": " << v << " " << units << endl;
        }
    }
}

