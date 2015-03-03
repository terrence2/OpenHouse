#!/usr/bin/bash
function kill_all_jobs { jobs -p | xargs kill; }
trap kill_all_jobs SIGINT

# Ensure that openhouse exists and is accessible.
# FIXME: this won't be needed after we remove zmq.
mkdir -p /var/run/openhouse
mkdir -p /var/run/openhouse/home

# Ensure that we have the log dir.
LOG_TIME=`date +%Y-%m-%d-%T`
LOGDIR="log/$LOG_TIME"
mkdir -p $LOGDIR
pushd log; rm -f latest; ln -s $LOG_TIME latest; popd

# Ensure that any subcommands we need are built.
make -C oh_home

# Enter the python virtualenv with our deps.
. .virtualenv3/bin/activate


{ node ./oh_home/build/main.js ./oh_home/eyrie.html | bunyan; } &
pid_home=$!
sleep 2; # FIXME: oh_home needs to have a server behind the named socks before the
         # other daemons can startup successfully.

./oh_hue/oh_hue.py --daemonize -L $LOGDIR/oh_hue.log &
pid_hue=$!

./oh_apply_scene/oh_apply_scene.py --daemonize -L $LOGDIR/oh_apply_scene.log &
pid_apply_scene=$!

./oh_wemo/oh_wemo.py -L $LOGDIR/oh_wemo.log &
pid_wemo=$!

./oh_infer_activity/oh_infer_activity.py -L $LOGDIR/oh_infer_activity.log &
pid_infer_activity=$!

{ pushd oh_web && ./oh_web_sabot.py; popd; } &
pid_web=$!


echo "pid home:           "$pid_home
echo "pid wemo:           "$pid_wemo
echo "pid infer activity: "$pid_infer_activity
echo "pid apply scene:    "$pid_apply_scene
echo "pid hue:            "$pid_hue
echo "pid web:            "$pid_web
wait $pid_web
wait $pid_hue
wait $pid_apply_scene
wait $pid_infer_activity
wait $pid_wemo
wait $pid_home

