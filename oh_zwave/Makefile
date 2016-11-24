CC=g++

CFLAGS=`ozw_config --Cflags` `ozw_config --Libs`
LIBS=-pthread -lboost_program_options

all: build/show_network build/configure_node

build/%.o : src/%.cpp
	${CC} -o $@ -c $< -Wall -g -O0 ${CFLAGS} ${LIBS}

build/show_network: build/show_network.o build/network.o
	${CC} -o $@ $^ -Wall -g -O0 ${CFLAGS} ${LIBS}

build/configure_node: build/configure_node.o build/network.o
	${CC} -o $@ $^ -Wall -g -O0 ${CFLAGS} ${LIBS}

clean:
	rm -rf build/*

