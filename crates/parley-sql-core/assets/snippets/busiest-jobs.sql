-- title: The busiest jobs right now
-- tags: did you know, work management
-- about: Active jobs using the most processor time and temporary storage (QSYS2.ACTIVE_JOB_INFO).
-- needs: IBM i 7.2
SELECT JOB_NAME, AUTHORIZATION_NAME, SUBSYSTEM, JOB_STATUS, CPU_TIME, TEMPORARY_STORAGE
  FROM TABLE (QSYS2.ACTIVE_JOB_INFO()) A
  ORDER BY CPU_TIME DESC
  FETCH FIRST ${rows:20} ROWS ONLY
