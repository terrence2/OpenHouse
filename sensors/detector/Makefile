# -Wall -pedantic
EXTRA_FLAGS=-O0 -g -W -Wextra -std=c++0x
OPENNI_CXXFLAGS=-Dlinux -D__x86_64__ -I/usr/include/ni
OPENNI_LIBS=-lOpenNI
LIBZMQ_CXXFLAGS=`pkg-config --cflags libzmq`
LIBZMQ_LIBS=`pkg-config --libs libzmq`
CXXFLAGS=$(EXTRA_FLAGS) $(OPENNI_CXXFLAGS) $(LIBZMQ_CXXFLAGS)
LIBS=$(OPENNI_LIBS) $(LIBZMQ_LIBS)

.SILENT:

OBJS=detector.o Kinect.o Network.o
PICKER_OBJS=picker.o Kinect.o Network.o
PROGS=detector picker tilt

all: $(PROGS)

clean:
	rm -f $(OBJS:.o=.dep) $(OBJS) $(PROGS) picker.o tilt.o
	rm -rf __pycache__

# pull in dependency info for *existing* .o files
-include $(OBJS:.o=.dep)

%.o: %.cpp
	echo $<
	g++ $(CXXFLAGS) -c $< -o $@
	g++ -MM $(CXXFLAGS) $< > $*.dep

detector: $(OBJS)
	echo $@
	g++ -o $@ $(OBJS) $(LIBS)

picker: $(PICKER_OBJS)
	echo $@
	g++ -o $@ $(PICKER_OBJS) $(LIBS)

tilt: tilt.cpp
	echo $@
	g++ -W -I/usr/include/libfreenect -o $@ $< -lfreenect -lfreenect_sync 
