-- title: Job queues
-- tags: did you know, jobs
-- about: Job queues with the jobs waiting in them and their subsystem, the fullest first (QSYS2.JOB_QUEUE_INFO).
-- needs: IBM i 7.3
SELECT JOB_QUEUE_LIBRARY, JOB_QUEUE_NAME, JOB_QUEUE_STATUS, NUMBER_OF_JOBS, SUBSYSTEM_NAME
  FROM QSYS2.JOB_QUEUE_INFO
  ORDER BY NUMBER_OF_JOBS DESC
