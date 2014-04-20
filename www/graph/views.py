# This Source Code Form is subject to the terms of the GNU General Public
# License, version 3. If a copy of the GPL was not distributed with this file,
# You can obtain one at https://www.gnu.org/licenses/gpl.txt.
from django.http import HttpResponse, HttpResponseNotFound
import os.path
import subprocess


def summary(request):
    sensor = 'rpi-nerve-bedroom'
    database = "/storage/raid/data/var/db/mcp/{}.rrd".format(sensor)
    return HttpResponse(subprocess.check_output([
        "rrdtool", "graph", "-",
        "-t", "Temperature And Humidity", "-w", "1280", "-h", "600",
        "DEF:celsius={}:temperature:AVERAGE".format(database),
        "DEF:humidity={}:humidity:AVERAGE".format(database),
        "CDEF:fahrenheit=celsius,9,*,5,/,32,+",
        "LINE1:humidity#0061cf:Humidity",
        "LINE1:fahrenheit#cf0061:Temperature"
    ]), content_type="image/png")


def database_for_sensor(sensor):
    database = "/storage/raid/data/var/db/mcp/{}.rrd".format(sensor)
    if not os.path.exists(database):
        return HttpResponseNotFound("I don't know about sensor {}".format(sensor))
    return database


def datasource_for_sensor(sensor):
    if 'humidity' in sensor:
        return 'humidity:AVERAGE'
    elif 'temperature' in sensor:
        return 'temperature:AVERAGE'
    elif 'motion' in sensor:
        return 'motion:AVERAGE'
    return HttpResponseNotFound("Can't derive the database " +
                                "for sensor {}.".format(sensor))


def datasource_wants_cf_conversion(datasource, GET):
    return datasource == 'temperature'


def color_for_sensor_number(i):
    colors = [
        '#0061cf',
        '#cf0061',
        '#00cf61',
        '#cf6100',
        '#61cf00',
        '#6100cf',
    ]
    return colors[i]


def legend_for_sensor(sensor):
    return sensor


def index(request):
    args = ['rrdtool', 'graph', '-']

    title = request.GET.get('title', 'Sensor Data')
    args += ['-t', title]

    width = request.GET.get('w', '1280')
    height = request.GET.get('h', '600')
    args += ['-w', width, '-h', height]

    sensors = request.GET.getlist('sensor')
    for i, sensor in enumerate(sensors):
        database = database_for_sensor(sensor)
        datasource = datasource_for_sensor(sensor)
        vname = 'vname_' + str(i)
        args.append("DEF:{}={}:{}".format(vname, database, datasource))
        if datasource_wants_cf_conversion(datasource, args):
            next_vname = 'vname2_' + str(i)
            args.append("CDEF:{}={},9,*,5,/,32,+".format(vname_next, vname))
            vname = vname_next
        color = color_for_sensor_number(i)
        legend = legend_for_sensor(sensor)
        args.append("LINE1:{}{}:{}".format(vname, color, legend))

    png = subprocess.check_output(args)
    return HttpResponse(png)

