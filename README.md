# MeetMerger

This is a tool to speed up merging of heats and generating heat sheets and timer sheets that account for the merging.
This is to solve the problem of having too few kids swimming in a multi-lane pool at a time during a swim meet.
It takes as input a Heat Sheet that's one column converted to PDF.

The input heat sheet should be structured as follows:

**#1 Boys 6 & Under 25m Freestyle**\
**Heat 1 of 1**\
1 Swimmer one &emsp;&emsp;&emsp;Age&emsp;Team&emsp;Entry time

The swim league this was built to support only allows printing heat sheets as PDF, so this tool works off of PDFs. 
A potential future feature will be to allow CSV as inputs as well.

### Usage:
Linux:
```bash
$ ./meetmerger-linux-x86_64 [--corrections <corrections-file>] [heat-sheet.pdf]
```
Windows:
```
> meetmerger-windows-x86_64.exe [--corrections <corrections-file>] [heat-sheet.pdf]
```

You can also just double-click the executable and load the files in the interface.

#### Step 1:
Select the files
