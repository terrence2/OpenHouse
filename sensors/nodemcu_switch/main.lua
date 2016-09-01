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

function rtrim(str)
    return str:match("(.-)%s*$")
end

function startswith(String,Start)
   return string.sub(String,1,string.len(Start))==Start
end

function send_event(url, status)
    local headers = 'Content-Type: application/x-www-form-urlencoded\r\n'
    http.post(url, headers, status, function(status_code, response_body)
        if status_code < 0 then
            print("http request failed with: "..status_code)
        end
    end)
end


-------------------------------------------------------------------------------
-- LED
LED_PIN = tonumber(rtrim(read_file("led.pin")))
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

gSSID = rtrim(read_file("wifi.ssid"))
gPassword = rtrim(read_file("wifi.password"))
wifi.sta.config(gSSID, gPassword)
wifi.sta.connect()


-------------------------------------------------------------------------------
-- Switches and Buttons
gAPITarget = rtrim(read_file("api-target.url"))
print("Using API Target URL: " .. gAPITarget)

function listen_debounced_pin(pin, handler)
    gpio.mode(pin, gpio.INT, gpio.PULLUP)

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

    --print("Listening with debounce to pin " .. pin)
end

function gpio2str(level)
    if level == gpio.HIGH then
        return "false"
    end
    return "true"
end

function create_openhouse_switch(pin_number, target_url)
    -- Create a new activation record so that target_url is stable despite the loop.
    listen_debounced_pin(pin_number, function(level)
        --print("would send event for pin " .. pin_number .. " value " .. gpio2str(level))
        send_event(target_url, gpio2str(level))
    end)
end

function create_openhouse_button(pin_number, event_name)
    -- Create a new activation record so that event_name is stable despite the loop.
    listen_debounced_pin(pin_number, function(level)
        if level == gpio.LOW then
            --print("would send event for pin " .. pin_number .. " value button-" .. pin_number)
            send_event(gAPITarget, event_name)
        end
    end)
end

function load_devices()
    local all_files = file.list();
    for filename, size in pairs(all_files) do
      if startswith(filename, "switch.") then
          print("Adding switch from file: " .. filename)
          local ending = string.sub(filename, string.len("switch.") + 1, -1)
          local pin_number = tonumber(ending)
          local target_url = rtrim(read_file(filename))
          create_openhouse_switch(pin_number, target_url)
      end
      if startswith(filename, "button.") then
          print("Adding button from file: " .. filename)
          local ending = string.sub(filename, string.len("button.") + 1, -1)
          local pin_number = tonumber(ending)
          local event_name = rtrim(read_file(filename))
          create_openhouse_button(pin_number, event_name)
      end
    end
end
load_devices()

-------------------------------------------------------------------------------
-- Done!
print("Boot Complete!")
print("  Heap available: "..node.heap())

