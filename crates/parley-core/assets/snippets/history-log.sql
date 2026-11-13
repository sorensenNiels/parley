-- title: The history log of the last hours
-- tags: did you know, messages
-- about: Messages in the system history log (QHST) since a number of hours ago (QSYS2.HISTORY_LOG_INFO).
-- needs: IBM i 7.3
SELECT MESSAGE_TIMESTAMP, MESSAGE_ID, FROM_USER, FROM_JOB, MESSAGE_TEXT
  FROM TABLE (QSYS2.HISTORY_LOG_INFO(CURRENT TIMESTAMP - ${hours:1} HOURS)) A
  ORDER BY MESSAGE_TIMESTAMP DESC
