-- title: Subsystems
-- tags: did you know, jobs
-- about: Subsystems with their status and active jobs, the busiest first (QSYS2.SUBSYSTEM_INFO).
-- needs: IBM i 7.4
SELECT SUBSYSTEM_DESCRIPTION_LIBRARY, SUBSYSTEM_DESCRIPTION, STATUS, CURRENT_ACTIVE_JOBS,
       MAXIMUM_ACTIVE_JOBS
  FROM QSYS2.SUBSYSTEM_INFO
  ORDER BY CURRENT_ACTIVE_JOBS DESC
