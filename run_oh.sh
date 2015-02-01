#!/usr/bin/bash
function kill_all_jobs { jobs -p | xargs kill; }
trap kill_all_jobs SIGINT

# Ensure that openhouse exists and is accessible.
mkdir -p /var/run/openhouse
mkdir -p /var/run/openhouse/home

# Ensure that any subcommands we need are built.
make -C oh_home

# Enter the python virtualenv with our deps.
. .virtualenv3/bin/activate


{ node ./oh_home/build/main.js ./oh_home/home.html | bunyan; } &
pid_home=$!
sleep 2; # FIXME: oh_home needs to have a server behind the named socks before the
         # other daemons can startup successfully.

./oh_hue/oh_hue.py --daemonize &
pid_hue=$!

./oh_scene/oh_scene.py --daemonize &
pid_scene=$!

{ pushd oh_web && ./oh_web_sabot.py; popd; } &
pid_web=$!


echo "pid home:  "$pid_home
echo "pid hue:   "$pid_hue
echo "pid scene: "$pid_scene
echo "pid web:   "$pid_web
wait $pid_web
wait $pid_scene
wait $pid_hue
wait $pid_home

