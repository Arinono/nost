-- Import the OBS Lua API
obs = obslua

-- Global variables
local source_name = ""
local stop_text = ""
local mode = ""
local a_mode = ""
local format = ""
local activated = false
local global = false
local timer_active = false
local settings_ = nil
local orig_time = 0
local cur_time = 0

-- Function to load script settings
function script_load(settings)
    settings_ = settings
end

-- Function to save script settings
function script_save(settings)
    settings_ = settings
end

-- Function to set default settings
function script_defaults(settings)
    obs.obs_data_set_default_string(settings, "source_name", "")
    obs.obs_data_set_default_string(settings, "stop_text", "")
    obs.obs_data_set_default_string(settings, "mode", "")
    obs.obs_data_set_default_string(settings, "a_mode", "")
    obs.obs_data_set_default_string(settings, "format", "")
    obs.obs_data_set_default_bool(settings, "activated", false)
    obs.obs_data_set_default_bool(settings, "global", false)
    obs.obs_data_set_default_bool(settings, "timer_active", false)
end

-- Function to describe the script
function script_description()
    return "This script handles various modes and timer functions for OBS."
end

-- Function to update the script settings
function script_update(settings)
    source_name = obs.obs_data_get_string(settings, "source_name")
    stop_text = obs.obs_data_get_string(settings, "stop_text")
    mode = obs.obs_data_get_string(settings, "mode")
    a_mode = obs.obs_data_get_string(settings, "a_mode")
    format = obs.obs_data_get_string(settings, "format")
    activated = obs.obs_data_get_bool(settings, "activated")
    global = obs.obs_data_get_bool(settings, "global")
    timer_active = obs.obs_data_get_bool(settings, "timer_active")
end

-- Function to define script properties
function script_properties()
    local props = obs.obs_properties_create()

    obs.obs_properties_add_text(props, "source_name", "Source Name", obs.OBS_TEXT_DEFAULT)
    obs.obs_properties_add_text(props, "stop_text", "Stop Text", obs.OBS_TEXT_DEFAULT)
    obs.obs_properties_add_text(props, "mode", "Mode", obs.OBS_TEXT_DEFAULT)
    obs.obs_properties_add_text(props, "a_mode", "A Mode", obs.OBS_TEXT_DEFAULT)
    obs.obs_properties_add_text(props, "format", "Format", obs.OBS_TEXT_DEFAULT)
    obs.obs_properties_add_bool(props, "activated", "Activated")
    obs.obs_properties_add_bool(props, "global", "Global")
    obs.obs_properties_add_bool(props, "timer_active", "Timer Active")

    return props
end

