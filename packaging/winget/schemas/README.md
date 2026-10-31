Unmodified JSON schemas, schema version 1.9.0, from Microsoft winget-cli (MIT).

Source commit: `c608376673762a10971e6bf2cbeedb6b6a7cae88`, resolved using
`git ls-remote https://github.com/microsoft/winget-cli.git refs/heads/master`.
Source directory: <https://github.com/microsoft/winget-cli/tree/c608376673762a10971e6bf2cbeedb6b6a7cae88/schemas/JSON/manifests/v1.9.0>.
Files are `manifest.{installer,version,defaultLocale}.1.9.0.json`, renamed only.
The upstream LICENSE is included. No external schema resolution is needed.

`python3 packaging/winget/validate.py DIRECTORY` checks all three YAML documents,
their identity, and their types. Requires existing PyYAML (MIT) and jsonschema
(MIT); no production dependency is added. `winget validate` remains the native
Windows acceptance check.
