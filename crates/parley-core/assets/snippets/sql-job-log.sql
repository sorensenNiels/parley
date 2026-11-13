-- title: The job log of this SQL job
-- tags: did you know, messages
-- about: The newest messages in the job log of the job running your SQL (QSYS2.JOBLOG_INFO); name another job as number/user/name.
-- needs: IBM i 7.1
SELECT MESSAGE_TIMESTAMP, MESSAGE_ID, MESSAGE_TYPE, SEVERITY, MESSAGE_TEXT
  FROM TABLE (QSYS2.JOBLOG_INFO('${job:*}')) A
  ORDER BY ORDINAL_POSITION DESC
  FETCH FIRST 100 ROWS ONLY
