-- title: Output queues
-- tags: did you know, spool
-- about: Output queues with their spooled files and writer, the fullest first (QSYS2.OUTPUT_QUEUE_INFO).
-- needs: IBM i 7.2
SELECT OUTPUT_QUEUE_LIBRARY_NAME, OUTPUT_QUEUE_NAME, OUTPUT_QUEUE_STATUS, NUMBER_OF_FILES, WRITER_NAME
  FROM QSYS2.OUTPUT_QUEUE_INFO
  ORDER BY NUMBER_OF_FILES DESC
