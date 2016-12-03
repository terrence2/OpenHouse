// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.

// Boost
#include <boost/program_options/cmdline.hpp>
#include <boost/program_options/option.hpp>
#include <boost/program_options/options_description.hpp>
#include <boost/program_options/parsers.hpp>
#include <boost/program_options/variables_map.hpp>

// Self
#include "network.h"

using std::cout;
using std::cerr;
using std::endl;
using std::string;

namespace po = boost::program_options;

static void onEvent(uint8_t id, uint8_t event, void* data) {
    cout << "Node Event on " << +id << ": " << +event << endl;
    int targetfd = (int)(size_t)data;
    uint8_t buf[2] = {id, event};
    ssize_t rv = write(targetfd, buf, 2);
    if (rv != 2) {
        cerr << "Write failure: " << rv << endl;
    }
}

int
main(int argc, char** argv)
{
    po::options_description desc("Show the current ZWave network.");
    desc.add_options()
        ("help,h", "Show these messages")
        ("verbose,V", "verbose logging")
        ("show,s", "show the network and exit")
        ("device,d", po::value<string>(), "the local controller")
        ("event-fd,e", po::value<int>(), "the file descriptor to write to")
    ;

    po::variables_map vm;
    po::store(po::parse_command_line(argc, argv, desc), vm);
    po::notify(vm);
    if (vm.count("help")) {
        cout << desc << endl;
        return 0;
    }

    std::vector<string> required{"device", "event-fd"};
    for (auto& name : required) {
        if (!vm.count(name)) {
            cout << "A " << name << " is required!" << endl;
            return 1;
        }
    }

    Network network(vm["device"].as<string>(), vm.count("verbose"));
    if (!network.init()) {
        cout << "Driver failed!" << endl;
        return 1;
    }
    cout << "Network iteration complete!" << endl;

    if (vm.count("show")) {
        network.show(true);
        return 0;
    }

    size_t event_fd = vm["event-fd"].as<int>();
    network.listen_events(onEvent, (void*)event_fd);
    while (true) {
        sleep(1000);
    }

    return 0;
}



