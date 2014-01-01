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


def index(request):
    sensor = request.GET['sensor']
    database = "/storage/raid/data/var/db/mcp/{}.rrd".format(sensor)
    if not os.path.exists(database):
        return HttpResponseNotFound("I don't know about sensor {}".format(sensor))
    width = request.GET.get('w', '1280')
    height = request.GET.get('h', '600')
    bare = bool(int(request.GET.get('bare', '0')))
    title = "Temperature and Humidity: " + sensor if not bare else ""
    humidity_legend = ":Humidity" if not bare else ""
    temp_legend = ":Temperature" if not bare else ""
    scale = request.GET.get('scale', 'fahrenheit')
    return HttpResponse(subprocess.check_output([
            "rrdtool", "graph", "-",
            "-t", title, "-w", width, "-h", height,
            "DEF:celsius={}:temperature:AVERAGE".format(database),
            "DEF:humidity={}:humidity:AVERAGE".format(database),
            "CDEF:fahrenheit=celsius,9,*,5,/,32,+",
            "LINE1:humidity#0061cf{}".format(humidity_legend),
            "LINE1:{}#cf0061{}".format(scale, temp_legend)
        ]), content_type="image/png")
