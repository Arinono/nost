obs = obslua
source_name = ""

stop_text = ""
mode = ""
a_mode = ""
format = ""
activated = false
global = false
timer_active = false
settings_ = nil
orig_time = 0
cur_time = 0
cur_ns = 0
up_when_finished = false
up = false
paused = false
switch_to_scene = false
next_scene = ""

hotkey_id_reset = obs.OBS_INVALID_HOTKEY_ID
hotkey_id_pause = obs.OBS_INVALID_HOTKEY_ID

function delta_time(year, month, day, hour, minute, second)
    local now = os.time()

    year = year == -1 and os.date("%Y", now) or year
    month = month == -1 and os.date("%m", now) or month
    day = day == -1 and os.date("%d", now) or day

    local future = os.time{year=year, month=month, day=day, hour=hour, min=minute, sec=second}
    local seconds = os.difftime(future, now)

    if seconds < 0 then
        seconds = seconds + 86400
    end

    return seconds * 1000000000
end

function set_time_text(ns, text)
    local ms = math.floor(ns / 1000000)
    local time_units = {
        { "%%0H", string.format("%02d", math.floor(ms / 3600000)) },
        { "%%0M", string.format("%02d", math.floor(ms / 60000)) },
        { "%%0S", string.format("%02d", math.floor(ms / 1000)) },
        { "%%0h", string.format("%02d", math.floor(ms / 3600000) % 24) },
        { "%%0m", string.format("%02d", math.floor(ms / 60000) % 60) },
        { "%%0s", string.format("%02d", math.floor(ms / 1000) % 60) },
        { "%%H", math.floor(ms / 3600000) },
        { "%%M", math.floor(ms / 60000) },
        { "%%S", math.floor(ms / 1000) },
        { "%%h", math.floor(ms / 3600000) % 24 },
        { "%%m", math.floor(ms / 60000) % 60 },
        { "%%s", math.floor(ms / 1000) % 60 },
        { "%%d", math.floor(ms / 86400000) },
    }

    for _, unit in ipairs(time_units) do
        text = string.gsub(text, unit[1], unit[2])
    end

    local decimal = string.format("%.3d", ms % 1000)
    local decimals = {
        { "%%3t", string.sub(decimal, 1, 3) },
        { "%%2t", string.sub(decimal, 1, 2) },
        { "%%t", string.sub(decimal, 1, 1) },
    }

    for _, dec in ipairs(decimals) do
        text = string.gsub(text, dec[1], dec[2])
    end

    local source = obs.obs_get_source_by_name(source_name)
    if source then
        local settings = obs.obs_data_create()
        obs.obs_data_set_string(settings, "text", text)
        obs.obs_source_update(source, settings)
        obs.obs_data_release(settings)
        obs.obs_source_release(source)
    end
end

function on_event(event)
    local events = {
        {obs.OBS_FRONTEND_EVENT_STREAMING_STARTED, "Streaming timer"},
        {obs.OBS_FRONTEND_EVENT_STREAMING_STOPPED, "Streaming timer"},
        {obs.OBS_FRONTEND_EVENT_RECORDING_STARTED, "Recording timer"},
        {obs.OBS_FRONTEND_EVENT_RECORDING_STOPPED, "Recording timer"}
    }

    for _, evt in ipairs(events) do
        if event == evt[1] then
            if mode == evt[2] then
                cur_time = 0
                stop_timer()
                start_timer()
            elseif event == obs.OBS_FRONTEND_EVENT_STREAMING_STOPPED or event == obs.OBS_FRONTEND_EVENT_RECORDING_STOPPED then
                stop_timer()
            end
        end
    end
end

function reset_timer()
    if mode == "Countdown" then
        cur_time = obs.obs_data_get_int(settings_, "duration") * 1000000000
    elseif mode == "Countup" then
        cur_time = obs.obs_data_get_int(settings_, "offset") * 1000000000
    elseif mode == "Specific time" then
        cur_time = delta_time(-1, -1, -1, obs.obs_data_get_int(settings_, "hour"), obs.obs_data_get_int(settings_, "minutes"), obs.obs_data_get_int(settings_, "seconds"))
    elseif mode == "Specific date and time" then
        cur_time = delta_time(obs.obs_data_get_int(settings_, "year"), obs.obs_data_get_int(settings_, "month"), obs.obs_data_get_int(settings_, "day"), obs.obs_data_get_int(settings_, "hour"), obs.obs_data_get_int(settings_, "minutes"), obs.obs_data_get_int(settings_, "seconds"))
    end
    cur_ns = cur_time
    set_time_text(cur_ns, format)
end

function script_tick(sec)
    if not timer_active then return end

    local delta = obs.os_gettime_ns() - orig_time
    cur_ns = mode == "Countup" or mode == "Streaming timer" or mode == "Recording timer" or up and cur_time + delta or cur_time - delta

    if cur_ns < 1 and (mode == "Countdown" or mode == "Specific time" or mode == "Specific date and time") then
        if not up_when_finished then
            stop_timer()
            if next_scene ~= "" and switch_to_scene then
                local next_scene_source = obs.obs_get_source_by_name(next_scene)
                obs.obs_source_release(next_scene_source)
				obs.obs_frontend_remove_event_callback(on_event)
				obs.obs_frontend_set_current_scene(next_scene_source)
				obs.obs_frontend_add_event_callback(on_event)
            else
                set_time_text(cur_ns, stop_text)
            end
            return
        else
            cur_time = 0
            up = true
            start_timer()
            return
        end
    end

    set_time_text(cur_ns, format)
end

function start_timer()
    timer_active = true
    orig_time = obs.os_gettime_ns()
end

function stop_timer()
    timer_active = false
end

function activate(activating)
    if activated == activating then return end

    if mode == "Streaming timer" or mode == "Recording timer" then return end

    activated = activating

    if activating and not global then
        script_update(settings_)
    end
end

function activate_signal(cd, activating)
    local source = obs.calldata_source(cd, "source")
    if source then
        if obs.obs_source_get_name(source) == source_name then
            activate(activating)
        end
    end
end

function on_source_activated(cd)
    activate_signal(cd, true)
end

function on_source_deactivated(cd)
    activate_signal(cd, false)
end

function reset_timer(pressed)
    if not pressed then return end

    if mode == "Streaming timer" or mode == "Recording timer" then return end

    script_update(settings_)
end

function on_pause(pressed)
    if not pressed then return end

    if mode == "Streaming timer" or mode == "Recording timer" then return end

    if cur_ns < 1 then
        reset_timer(true)
    end

    if timer_active then
        stop_timer()
        cur_time = cur_ns
        paused = true
    else
        start_timer()
        paused = false
    end
end

function pause_button_clicked(props, p)
    on_pause(true)
    return true
end

function reset_button_clicked(props, p)
    reset_timer(true)
    return true
end

function settings_modified(props, prop, settings)
    local mode_setting = obs.obs_data_get_string(settings, "mode")
    local enable_scene_switch = obs.obs_data_get_bool(settings, "switch_to_scene")

    obs.obs_property_set_enabled(obs.obs_properties_get(props, "next_scene"), enable_scene_switch)

    local visibilities = {
        {"Countdown", true, false, false, false, false, false, false, true, true, true, true, true, true},
        {"Countup", false, true, false, false, false, false, false, false, true, true, true, false, false, true},
        {"Specific time", false, false, true, true, true, true, true, true, true, true, true, true, true, true},
        {"Specific date and time", false, false, true, true, true, true, true, true, true, true, true, true, true, true},
        {"Streaming timer", false, false, false, false, false, false, false, false, false, false, false, false, false, true},
        {"Recording timer", false, false, false, false, false, false, false, false, false, false, false, false, false, true}
    }

    for _, vis in ipairs(visibilities) do
        if mode_setting == vis[1] then
            obs.obs_property_set_visible(obs.obs_properties_get(props, "duration"), vis[2])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "offset"), vis[3])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "year"), vis[4])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "month"), vis[5])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "day"), vis[6])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "hour"), vis[7])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "minutes"), vis[8])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "seconds"), vis[9])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "stop_text"), vis[10])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "pause_button"), vis[11])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "reset_button"), vis[12])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "a_mode"), vis[13])
            obs.obs_property_set_visible(obs.obs_properties_get(props, "up_when_finished"), vis[14])
        end
    end

    return true
end

function script_properties()
    local props = obs.obs_properties_create()

    local list_property = obs.obs_properties_add_list(props, "source", "Text Source", obs.OBS_COMBO_TYPE_EDITABLE, obs.OBS_COMBO_FORMAT_STRING)
    local sources = obs.obs_enum_sources()

    if sources then
        for _, source in ipairs(sources) do
            local source_id = obs.obs_source_get_unversioned_id(source)
            if source_id == "text_gdiplus" or source_id == "text_ft2_source" then
                obs.obs_property_list_add_string(list_property, obs.obs_source_get_name(source), obs.obs_source_get_name(source))
            end
        end
    end

    obs.source_list_release(sources)

    local modes = {"Countdown", "Countup", "Specific time", "Specific date and time", "Streaming timer", "Recording timer"}
    local p_mode = obs.obs_properties_add_list(props, "mode", "Mode", obs.OBS_COMBO_TYPE_LIST, obs.OBS_COMBO_FORMAT_STRING)

    for _, mode in ipairs(modes) do
        obs.obs_property_list_add_string(p_mode, mode, mode)
    end
    obs.obs_property_set_modified_callback(p_mode, settings_modified)

    obs.obs_properties_add_int(props, "duration", "Duration (seconds)", 1, 86400, 1)
    obs.obs_properties_add_int(props, "offset", "Offset (seconds)", 0, 86400, 1)

    obs.obs_properties_add_int(props, "year", "Year (-1 = current)", -1, 3000, 1)
    obs.obs_properties_add_int(props, "month", "Month (-1 = current)", -1, 12, 1)
    obs.obs_properties_add_int(props, "day", "Day (-1 = current)", -1, 31, 1)

    obs.obs_properties_add_int_slider(props, "hour", "Hour", 0, 23, 1)
    obs.obs_properties_add_int_slider(props, "minutes", "Minute", 0, 59, 1)
    obs.obs_properties_add_int_slider(props, "seconds", "Second", 0, 59, 1)

    obs.obs_properties_add_text(props, "format", "Text format", obs.OBS_TEXT_DEFAULT)
    obs.obs_properties_add_text(props, "stop_text", "Stop text", obs.OBS_TEXT_DEFAULT)

	local p_act_mode = obs.obs_properties_add_list(props, "a_mode", "Activation mode", obs.OBS_COMBO_TYPE_EDITABLE, obs.OBS_COMBO_FORMAT_STRING)
	obs.obs_property_list_add_string(p_act_mode, "Global (timer always active)", "global")
	obs.obs_property_list_add_string(p_act_mode, "Start timer on activation", "start_reset")

    obs.obs_properties_add_button(props, "reset_button", "Reset Timer", reset_button_clicked)
    obs.obs_properties_add_button(props, "pause_button", "Start/Stop Timer", pause_button_clicked)

    obs.obs_properties_add_bool(props, "up_when_finished", "Countup after finish")

    obs.obs_properties_add_bool(props, "switch_to_scene", "Switch to scene")
    local list_next_scene = obs.obs_properties_add_list(props, "next_scene", "Next Scene", obs.OBS_COMBO_TYPE_EDITABLE, obs.OBS_COMBO_FORMAT_STRING)

    local scenes = obs.obs_frontend_get_scenes()
    if scenes then
        for _, scene in ipairs(scenes) do
            obs.obs_property_list_add_string(list_next_scene, obs.obs_source_get_name(scene), obs.obs_source_get_name(scene))
        end
    end
    obs.source_list_release(scenes)

    settings_modified(props, nil, settings_)

    return props
end

function script_update(settings)
    stop_timer()
    up = false

    settings_ = settings
    source_name = obs.obs_data_get_string(settings, "source")
    mode = obs.obs_data_get_string(settings, "mode")
    a_mode = obs.obs_data_get_string(settings, "a_mode")
    global = a_mode == "Global (timer always active)"
    stop_text = obs.obs_data_get_string(settings, "stop_text")
    format = obs.obs_data_get_string(settings, "format")
    switch_to_scene = obs.obs_data_get_bool(settings, "switch_to_scene")
    next_scene = obs.obs_data_get_string(settings, "next_scene")
    up_when_finished = obs.obs_data_get_bool(settings, "up_when_finished")

    if mode == "Countdown" then
        cur_time = obs.obs_data_get_int(settings, "duration") * 1000000000
        cur_ns = cur_time
    elseif mode == "Countup" then
        cur_time = obs.obs_data_get_int(settings, "offset") * 1000000000
        cur_ns = cur_time
    elseif mode == "Specific time" then
        cur_time = delta_time(-1, -1, -1, obs.obs_data_get_int(settings, "hour"), obs.obs_data_get_int(settings, "minutes"), obs.obs_data_get_int(settings, "seconds"))
        cur_ns = cur_time
    elseif mode == "Specific date and time" then
        cur_time = delta_time(obs.obs_data_get_int(settings, "year"), obs.obs_data_get_int(settings, "month"), obs.obs_data_get_int(settings, "day"), obs.obs_data_get_int(settings, "hour"), obs.obs_data_get_int(settings, "minutes"), obs.obs_data_get_int(settings, "seconds"))
        cur_ns = cur_time
    elseif mode == "Streaming timer" then
        global = true
        local streaming = obs.obs_frontend_streaming_active()
        if streaming then
            cur_time = 0
        end
    elseif mode == "Recording timer" then
        global = true
        local recording = obs.obs_frontend_recording_active()
        if recording then
            cur_time = 0
        end
    end

    set_time_text(cur_time, format)
    if global == false and paused == false then
        start_timer()
    end
end

function script_defaults(settings)
    obs.obs_data_set_default_string(settings, "source", "Timer")
    obs.obs_data_set_default_int(settings, "duration", 60)
    obs.obs_data_set_default_string(settings, "mode", "Countdown")
    obs.obs_data_set_default_string(settings, "format", "%0m:%0s")
    obs.obs_data_set_default_string(settings, "stop_text", "00:00")
    obs.obs_data_set_default_bool(settings, "switch_to_scene", false)
    obs.obs_data_set_default_string(settings, "a_mode", "Start timer on activation")
end

function script_save(settings)
    local hotkey_save_array = obs.obs_hotkey_save(hotkey_id_reset)
    obs.obs_data_set_array(settings, "reset_hotkey", hotkey_save_array)
    obs.obs_data_array_release(hotkey_save_array)

    hotkey_save_array = obs.obs_hotkey_save(hotkey_id_pause)
    obs.obs_data_set_array(settings, "pause_hotkey", hotkey_save_array)
    obs.obs_data_array_release(hotkey_save_array)
end

function script_load(settings)
    local hotkey_save_array = obs.obs_data_get_array(settings, "reset_hotkey")
    obs.obs_hotkey_load(hotkey_id_reset, hotkey_save_array)
    obs.obs_data_array_release(hotkey_save_array)

    hotkey_save_array = obs.obs_data_get_array(settings, "pause_hotkey")
    obs.obs_hotkey_load(hotkey_id_pause, hotkey_save_array)
    obs.obs_data_array_release(hotkey_save_array)

    hotkey_id_reset = obs.obs_hotkey_register_frontend("reset_timer", "Reset Timer", reset_timer)
    hotkey_id_pause = obs.obs_hotkey_register_frontend("pause_timer", "Pause Timer", on_pause)

	local sh = obs.obs_get_signal_handler()
	obs.signal_handler_connect(sh, "source_activate", on_source_activated)
	obs.signal_handler_connect(sh, "source_deactivate", on_source_deactivated)

    obs.obs_frontend_add_event_callback(on_event)

    settings_ = settings
    script_update(settings)
end


function script_description()
    return "This script provides various countdown and countup timer functionalities with support for OBS scene switching upon timer completion.\n\n" ..
           "Modes:\n" ..
           "Countdown - Counts down from a specified duration.\n" ..
           "Countup - Counts up from a specified offset.\n" ..
           "Specific time - Counts down to a specified time within the same day.\n" ..
           "Specific date and time - Counts down to a specific date and time.\n" ..
           "Streaming timer - Automatically starts and stops with streaming.\n" ..
           "Recording timer - Automatically starts and stops with recording."
end
