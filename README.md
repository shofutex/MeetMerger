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
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,1,"Roe, Sam EXH",8,Dolphins,1:02.34
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,2,"Smith, John",7,Dolphins,NT
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,3,—,,,
#1 Boys 8 & Under 25m Freestyle,Heat 1 of 1,4,"Body, Any",8,Sharks,1:01.32
#2 Girls 8 & Under 25m Freestyle,Heat 1 of 1,4,"Doe, Jane",7,Sharks,32.10
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

#### Step 2:
Review the merged heat. You have the option to change the name if desired
![Screenshot of the mixed heat](/images/mixed_heat.png)

#### Step 3:
When you've merged all of the heats you plan to merge, review the final meet overview:
![Screenshot of the meet overview](/images/final_preview.png)

#### Step 4:
Finally, you have the option to generate a new heat sheet with the mixed heats and timer sheets that match. 
You have the option to enter abbreviations for the teams if desired.

Our team has an individual medley (IM) carnival as one of the last meets and so we put the IM events first. There's an option to start with a higher level events in the heat sheet and timer sheets as well.
![Screenshot of the final export page](/images/export.png)
