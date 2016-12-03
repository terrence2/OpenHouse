# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
CC=g++

CFLAGS=`ozw_config --Cflags` `ozw_config --Libs`
LIBS=-pthread -lboost_program_options

all: build/oh_zwave

build/%.o : src/%.cpp Makefile
	${CC} -o $@ -c $< -Wall -g -O0 ${CFLAGS} ${LIBS}

build/oh_zwave: build/oh_zwave.o build/network.o
	${CC} -o $@ $^ -Wall -g -O0 ${CFLAGS} ${LIBS}

clean:
	rm -rf build/*

