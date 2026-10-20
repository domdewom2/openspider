# User-Agent page (M2-U13)
config-ua-desc = The preset brings its own user agent and robots.txt token. You can override both.
config-ua-search = preset bot googlebot token
config-ua-custom = Custom
config-f-user-agent-preset = Preset
config-f-user-agent-http = HTTP user agent
config-f-user-agent-robots-token = robots.txt user agent (token)
config-hint-robots-token = The token decides which group of robots.txt applies, for example Googlebot.
config-ua-effective = In effect: { $ua } · robots.txt token { $token }
config-ua-missing-token = A custom user agent without robots.txt token only uses the group "User-agent: *".
