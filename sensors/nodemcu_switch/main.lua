-------------------------------------------------------------------------------
-- Global Data
gTimerIndex = 1


-------------------------------------------------------------------------------
-- Utility
function read_file(filename)
    file.open(filename, "r")
    local content = file.read()
    file.close()
    return content
end

function startswith(String,Start)
   return string.sub(String,1,string.len(Start))==Start
end


-------------------------------------------------------------------------------
-- LED
LED_PIN = 3     -- pin 0 for arduino / PCB
gpio.mode(LED_PIN, gpio.OUTPUT)

function led_on() gpio.write(LED_PIN, gpio.LOW) end
function led_off() gpio.write(LED_PIN, gpio.HIGH) end


-------------------------------------------------------------------------------
-- WiFi
led_on()
wifi.setmode(wifi.STATION)
wifi.sta.eventMonReg(wifi.STA_IDLE, function() print("STATION_IDLE") end)
wifi.sta.eventMonReg(wifi.STA_CONNECTING, function() print("STATION_CONNECTING"); led_on() end)
wifi.sta.eventMonReg(wifi.STA_WRONGPWD, function() print("STATION_WRONG_PASSWORD"); led_on() end)
wifi.sta.eventMonReg(wifi.STA_APNOTFOUND, function() print("STATION_NO_AP_FOUND"); led_on() end)
wifi.sta.eventMonReg(wifi.STA_FAIL, function() print("STATION_CONNECT_FAIL"); led_on() end)
wifi.sta.eventMonReg(wifi.STA_GOTIP, function() print("STATION_GOT_IP"); led_off() end)
wifi.sta.eventMonStart(1000)

gSSID = read_file("wifi.ssid")
gPassword = read_file("wifi.password")
wifi.sta.config(gSSID, gPassword)
wifi.sta.connect()


-------------------------------------------------------------------------------
-- Switches
function create_switch(pin, handler)
    gpio.mode(pin, gpio.INT, gpio.FLOAT)

    local last_debounce_level = gpio.LOW
    local timer_index = gTimerIndex
    gTimerIndex = gTimerIndex + 1

    tmr.register(timer_index, 5, tmr.ALARM_SEMI, function()
        local new_level = gpio.read(pin)
        if new_level == last_debounce_level then
            handler(new_level)
        else
            last_debounce_level = new_level
            tmr.start(timer_index)
        end
    end)

    gpio.trig(pin, "both", function(level)
        last_debounce_level = level
        tmr.start(timer_index)
    end)
end

function gpio2str(level)
    if level == gpio.HIGH then
        return "false"
    end
    return "true"
end

function send_event(url, status)
    local headers = 'Content-Type: application/x-www-form-urlencoded\r\n'
    http.post(url, headers, status, function(status_code, response_body)
        if status_code < 0 then
            print("http request failed with: "..status_code)
        end
    end)
end

function create_openhouse_switch(pin_number, target_url)
    -- Create a new activation record so that target_url is stable despite the loop.
    create_switch(pin_number, function(level)
        send_event(target_url, gpio2str(level))
    end)
end

function load_devices()
    local all_files = file.list();
    for filename, size in pairs(all_files) do
      if startswith(filename, "switch.") then
          local ending = string.sub(filename, string.len("switch.") + 1, -1)
          local pin_number = tonumber(ending)
          local target_url = read_file(filename)
          create_openhouse_switch(pin_number, target_url)
      end
    end
end
load_devices()

-------------------------------------------------------------------------------
-- Done!
print("Boot Complete!")
print("  Heap available: "..node.heap())

