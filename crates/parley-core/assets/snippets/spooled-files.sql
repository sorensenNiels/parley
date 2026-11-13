-- title: Spooled files of a user
-- tags: did you know, output
-- about: A user's spooled files, newest first (QSYS2.SPOOLED_FILE_INFO, which looks only at that user's files); leave the user empty for your own.
-- needs: IBM i 7.3
SELECT SPOOLED_FILE_NAME, USER_DATA, CREATION_TIMESTAMP, STATUS, TOTAL_PAGES, OUTPUT_QUEUE, QUALIFIED_JOB_NAME
  FROM TABLE(QSYS2.SPOOLED_FILE_INFO(USER_NAME => COALESCE(NULLIF(UPPER('${user:}'), ''), '*CURRENT')))
  ORDER BY CREATION_TIMESTAMP DESC
