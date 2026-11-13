-- title: The library list of this SQL job
-- tags: did you know, work management
-- about: The libraries an unqualified name is looked for in, in order (QSYS2.LIBRARY_LIST_INFO).
-- needs: IBM i 7.1
SELECT ORDINAL_POSITION, SCHEMA_NAME, TYPE
  FROM QSYS2.LIBRARY_LIST_INFO
  ORDER BY ORDINAL_POSITION
