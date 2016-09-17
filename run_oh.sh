#!/usr/bin/bash
# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
function kill_all_jobs { jobs -p | xargs kill; }
trap kill_all_jobs SIGINT

# Ensure that we have the log dir.
LOG_TIME=`date +%Y-%m-%d-%T`
LOGDIR="log/$LOG_TIME"
mkdir -p $LOGDIR
pushd log; rm -f latest; ln -s $LOG_TIME latest; popd

PORT=8185

# Ensure that any subcommands we need are built.
pushd oh_db; cargo build --release; popd
make -C oh_home

# Enter the python virtualenv with our deps.
. .venv/bin/activate

# Start the main database server and populate it.
./oh_db/target/release/oh_db \
    -l info -L $LOGDIR/oh_db.log \
    -a 127.0.0.1 -p $PORT \
    -C CA/intermediate/certs/chain.cert.pem \
    -c CA/intermediate/certs/oh_db.cert.pem \
    -k CA/intermediate/private/oh_db.key.pem &
pid_db=$!
./oh_populate/oh_populate.py \
    -l INFO \
    -L $LOGDIR/oh_populate.log \
    -A 127.0.0.1 \
    -P $PORT \
    -C CA/intermediate/certs/chain.cert.pem \
    -c CA/intermediate/certs/oh_populate.cert.pem \
    -k CA/intermediate/private/oh_populate.key.pem \
    --config $1

./oh_hue/oh_hue.py \
    -l INFO \
    -L $LOGDIR/oh_hue.log \
    -A 127.0.0.1 \
    -P $PORT \
    -C CA/intermediate/certs/chain.cert.pem \
    -c CA/intermediate/certs/oh_hue.cert.pem \
    -k CA/intermediate/private/oh_hue.key.pem &
pid_hue=$!

./oh_color/oh_color.py \
    -l INFO \
    -L $LOGDIR/oh_color.log \
    -A 127.0.0.1 \
    -P $PORT \
    -C CA/intermediate/certs/chain.cert.pem \
    -c CA/intermediate/certs/oh_color.cert.pem \
    -k CA/intermediate/private/oh_color.key.pem &
pid_color=$!

./oh_formula/oh_formula.py \
    -l INFO \
    -L $LOGDIR/oh_formula.log \
    -A 127.0.0.1 \
    -P $PORT \
    -C CA/intermediate/certs/chain.cert.pem \
    -c CA/intermediate/certs/oh_formula.cert.pem \
    -k CA/intermediate/private/oh_formula.key.pem &
pid_formula=$!

./oh_button/oh_button.py \
    -l INFO \
    -L $LOGDIR/oh_button.log \
    -A 127.0.0.1 \
    -P $PORT \
    -C CA/intermediate/certs/chain.cert.pem \
    -c CA/intermediate/certs/oh_button.cert.pem \
    -k CA/intermediate/private/oh_button.key.pem &
pid_button=$!

# Note that this is for debugging only.
./oh_rest/oh_rest.py \
    -l INFO \
    -L $LOGDIR/oh_rest.log \
    -A 127.0.0.1 \
    -P $PORT \
    -C CA/intermediate/certs/chain.cert.pem \
    -c CA/intermediate/certs/oh_rest.cert.pem \
    -k CA/intermediate/private/oh_rest.key.pem \
    -p 8081 &
pid_rest=$!

#./oh_hue/oh_hue.py -L $LOGDIR/oh_hue.log -P $PORT &
#pid_hue=$!

#./oh_apply_scene/oh_apply_scene.py -L $LOGDIR/oh_apply_scene.log -P $PORT &
#pid_apply_scene=$!

#./oh_apply_sensor/oh_apply_sensor.py -L $LOGDIR/oh_apply_sensor.log -P $PORT &
#pid_apply_sensor=$!

#./oh_wemo/oh_wemo.py -L $LOGDIR/oh_wemo.log -P $PORT &
#pid_wemo=$!

#./oh_motion_filter/oh_motion_filter.py -L $LOGDIR/oh_motion_filter.log -P $PORT &
#pid_motion_filter=$!

#./oh_infer_activity/oh_infer_activity.py -l INFO -L $LOGDIR/oh_infer_activity.log -P $PORT &
#pid_infer_activity=$!

#./oh_alarm/oh_alarm.py -l INFO -L $LOGDIR/oh_alarm.log -P $PORT &
#pid_alarm=$!

#{ pushd oh_web && ./oh_web_sabot.py -L ../$LOGDIR/oh_web.log -p 8080 -P $PORT; popd; } &
#pid_web=$!


echo "pid db:             "$pid_db
echo "pid button:         "$pid_button
echo "pid formula:        "$pid_formula
echo "pid color:          "$pid_color
#echo "pid wemo:           "$pid_wemo
#echo "pid motion filter:  "$pid_motion_filter
#echo "pid infer activity: "$pid_infer_activity
#echo "pid apply scene:    "$pid_apply_scene
#echo "pid apply sensor:   "$pid_apply_sensor
#echo "pid hue:            "$pid_hue
#echo "pid alarm:          "$pid_alarm
echo "pid rest:           "$pid_rest
#echo "pid web:            "$pid_web
#wait $pid_web
wait $pid_rest
#wait $pid_alarm
#wait $pid_hue
#wait $pid_apply_sensor
#wait $pid_apply_scene
#wait $pid_infer_activity
#wait $pid_motion_filter
#wait $pid_wemo
wait $pid_color
wait $pid_formula
wait $pid_button
wait $pid_db

