# MeetMerger

This is a tool to speed up merging of heats and generating heat sheets and timer sheets that account for the merging.
This is to solve the problem of having too few kids swimming in a multi-lane pool at a time during a swim meet.
It takes as input a Heat Sheet that's one column converted to PDF.

The input heat sheet should be structured as follows (PDF):

**#1 Boys 6 & Under 25m Freestyle**\
**Heat 1 of 1**\
1 Swimmer one &emsp;&emsp;&emsp;Age&emsp;Team&emsp;Entry time

Or as follows for CSV:
```
event name,heat,lane,name,age,team,entry time
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 2,1,"Roe, Sam EXH",8,Dolphins,1:02.34
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 2,3,"Smith, John",7,Dolphins,NT
#1 Boys 8 & Under 25m Freestyle,Heat 2 of 2,2,—,,,
#2 Girls 10 & Under 50m Freestyle,Heat 1 of 1,4,"Doe, Jane",9,Sharks,32.10
```


The tool will treat a .csv file as a CSV and anything else as a PDF.

### Usage:
Linux:
```bash
$ ./meetmerger-linux-x86_64 [--corrections <corrections-file>] [heat-sheet]
```
Windows:
```
> meetmerger-windows-x86_64.exe [--corrections <corrections-file>] [heat-sheet]
```

You can also just double-click the executable and load the files in the interface.

#### Step 1:
Select the heats to merge
![Screenshot of selecting the heats to merge](/images/heat_selection.png)
