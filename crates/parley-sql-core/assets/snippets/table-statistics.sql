-- title: Rows and deleted rows of a library's tables
-- tags: did you know, database
-- about: Each table's rows, deleted rows and size, to find files worth reorganizing (QSYS2.SYSTABLESTAT).
-- needs: IBM i 7.1
SELECT TABLE_NAME, NUMBER_ROWS, NUMBER_DELETED_ROWS, DATA_SIZE
  FROM QSYS2.SYSTABLESTAT
  WHERE TABLE_SCHEMA = UPPER('${library:QGPL}')
  ORDER BY NUMBER_DELETED_ROWS DESC
