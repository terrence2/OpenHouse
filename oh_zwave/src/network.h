// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
#pragma once

// Std
#include <condition_variable>
#include <iostream>
#include <mutex>
#include <string>
#include <vector>
#include <unordered_map>
// OpenZWave
#include <Notification.h>
#include <value_classes/Value.h>


class Network
{
    std::string device_name;
    uint32_t home_id;

    // Initialization.
    std::mutex lock;
    std::condition_variable guard_done;
    bool done;
    bool failed;

    // Runtime Callbacks.
    using ListenerEventCallbackType = void (*)(uint8_t id, uint8_t event, void* data);
    ListenerEventCallbackType event_listener;
    void* event_listener_data;

    struct Node
    {
        uint8_t id;
        std::string productName;
        std::string productType;
        std::string productId;
        std::string manufacturerName;
        std::string manufacturerId;
        std::vector<OpenZWave::ValueID> values;

        Node() : id(0) {}
        Node(uint8_t c) : id(c) {}

        void showBasicInfo();
        void showValueGenre(const std::string& name, OpenZWave::ValueID::ValueGenre genre);
    };
    std::unordered_map<uint8_t, Node> nodes;

    static void InitHandler(OpenZWave::Notification const* _notification, void* _context);
    static void ListenEventsHandler(OpenZWave::Notification const* notification, void* _context);

 public:
    Network(std::string dev_name, bool verbose);
    ~Network();
    bool init();
    void show(bool verbose);

    bool listen_events(ListenerEventCallbackType callback, void* data);
};

