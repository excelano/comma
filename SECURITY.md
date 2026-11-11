# Security Policy

## Reporting a vulnerability

Please report suspected vulnerabilities privately, using the **Report a vulnerability** button on the Security tab of the repository concerned. If that button is not there, or you would rather not go through GitHub, email david.anderson@excelano.com instead. I aim to respond within seven days.

Please do not open public issues for security problems.

## Supported versions

The latest release receives security fixes. Older versions are not supported. A fix ships in a new release; update the way you installed.

## What Comma can access

Comma is a desktop editor that runs locally on your machine. It reads the delimited file you open, holds it in memory for the duration of the session, and writes it back only when you save. Export writes a new file at a location you choose. Comma makes no network calls of any kind, has no auth layer, and can only read and write files your operating-system user already has access to.

## What Comma stores

Comma stores window geometry in GSettings under `com.excelano.Comma`. Nothing else: no history file, no recent-files cache beyond what the desktop provides, no telemetry, no analytics, no remote logging.

## Handling untrusted files

A delimited file is data, not code. Comma never evaluates cell contents — there are no formulas, no macros, and no scripting, so the class of attack that targets spreadsheet formula evaluation does not apply. Cell values are always treated as opaque text.
