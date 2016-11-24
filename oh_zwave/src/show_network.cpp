// This Source Code Form is subject to the terms of the GNU General Public
// License, version 3. If a copy of the GPL was not distributed with this file,
// You can obtain one at https://www.gnu.org/licenses/gpl.txt.
#include "network.h"

// Boost
#include <boost/program_options/cmdline.hpp>
#include <boost/program_options/option.hpp>
#include <boost/program_options/options_description.hpp>
#include <boost/program_options/parsers.hpp>
#include <boost/program_options/variables_map.hpp>

using std::cout;
using std::endl;
using std::string;

namespace po = boost::program_options;

int
main(int argc, char** argv)
{
    po::options_description desc("Show the current ZWave network.");
    desc.add_options()
        ("help,h", "Show these messages")
        ("verbose,V", "verbose logging")
        ("device,d", po::value<string>(), "the local controller")
    ;
    po::variables_map vm;
    po::store(po::parse_command_line(argc, argv, desc), vm);
    po::notify(vm);
    if (vm.count("help")) {
        cout << desc << endl;
        return 0;
    }
    if (!vm.count("device")) {
        cout << "A device is required!" << endl;
        return 1;
    }

    Network network(vm["device"].as<string>(), vm.count("verbose") > 1);
    if (!network.init()) {
        cout << "Driver failed!" << endl;
        return 1;
    }
    cout << "Network iteration complete!" << endl;

    network.show(vm.count("verbose") > 0);
    return 0;
}

