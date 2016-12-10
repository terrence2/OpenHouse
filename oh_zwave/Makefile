# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
CC=g++

OPTFLAGS=-O2
#OPTFLAGs=-g -O0
CFLAGS=`ozw_config --Cflags` `ozw_config --Libs`
LIBS=-pthread -lboost_program_options

all: build/oh_zwave

build:
	mkdir -p build

build/%.o : src/%.cpp Makefile build
	${CC} -o $@ -c $< -Wall ${OPTFLAGS} ${CFLAGS} ${LIBS}

build/oh_zwave: build/oh_zwave.o build/network.o
	${CC} -o $@ $^ -Wall ${OPTFLAGS} ${CFLAGS} ${LIBS}

clean:
	rm -rf build

