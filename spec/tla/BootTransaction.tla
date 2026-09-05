------------------------- MODULE BootTransaction -------------------------
EXTENDS Naturals, FiniteSets
CONSTANTS Entries, MaxTries
NoEntry == CHOOSE x : x \notin Entries
VARIABLES phase, selected, triesLeft, lastGood, committed
vars == <<phase, selected, triesLeft, lastGood, committed>>
Phases == {"Idle","Selected","Validated","Staged","Launched","Confirmed","Failed","Recovery"}
Bootable(e) == e \in Entries /\ triesLeft[e] > 0
StartAttempt == \E e \in Entries : Select(e)
TypeOK == /\ phase \in Phases
          /\ selected \in Entries \cup {NoEntry}
          /\ triesLeft \in [Entries -> 0..MaxTries]
          /\ lastGood \in Entries \cup {NoEntry}
          /\ committed \in Entries \cup {NoEntry}
Init == /\ phase = "Idle" /\ selected = NoEntry
        /\ triesLeft = [e \in Entries |-> MaxTries]
        /\ lastGood = NoEntry /\ committed = NoEntry
Select(e) == /\ phase = "Idle" /\ Bootable(e)
             /\ selected' = e /\ phase' = "Selected"
             /\ UNCHANGED <<triesLeft, lastGood, committed>>
Validate == /\ phase = "Selected" /\ Bootable(selected) /\ phase' = "Validated"
            /\ UNCHANGED <<selected, triesLeft, lastGood, committed>>
Stage == /\ phase = "Validated"
         /\ triesLeft' = [triesLeft EXCEPT ![selected] = @ - 1]
         /\ phase' = "Staged" /\ UNCHANGED <<selected, lastGood, committed>>
Launch == /\ phase = "Staged" /\ phase' = "Launched"
          /\ UNCHANGED <<selected, triesLeft, lastGood, committed>>
ConfirmSuccess == /\ phase = "Launched"
                  /\ committed' = selected /\ lastGood' = selected
                  /\ triesLeft' = [triesLeft EXCEPT ![selected] = MaxTries]
                  /\ phase' = "Confirmed" /\ UNCHANGED <<selected>>
DetectFailure == /\ phase = "Launched" /\ phase' = "Failed"
                 /\ UNCHANGED <<selected, triesLeft, lastGood, committed>>
RollBack == /\ phase = "Failed" /\ \E e \in Entries : Bootable(e)
            /\ selected' = IF Bootable(lastGood) THEN lastGood
                          ELSE CHOOSE e \in Entries : Bootable(e)
            /\ phase' = "Selected" /\ UNCHANGED <<triesLeft, lastGood, committed>>
Recover == /\ phase = "Failed" /\ \A e \in Entries : ~Bootable(e)
           /\ phase' = "Recovery" /\ UNCHANGED <<selected, triesLeft, lastGood, committed>>
Next == StartAttempt \/ Validate \/ Stage \/ Launch
        \/ ConfirmSuccess \/ DetectFailure \/ RollBack \/ Recover
Fairness == /\ WF_vars(StartAttempt) /\ WF_vars(Validate) /\ WF_vars(Stage)
            /\ WF_vars(Launch) /\ WF_vars(DetectFailure)
            /\ WF_vars(RollBack) /\ WF_vars(Recover)
Spec == Init /\ [][Next]_vars /\ Fairness
NoDeadEnd == (phase = "Failed") => (ENABLED RollBack \/ ENABLED Recover)
Resolved == phase \in {"Confirmed","Recovery"}
EventuallyResolved == <>Resolved
THEOREM Spec => []TypeOK
THEOREM Spec => []NoDeadEnd
THEOREM Spec => EventuallyResolved
==========================================================================
