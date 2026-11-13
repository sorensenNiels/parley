-- title: The largest objects in a library
-- tags: did you know, storage
-- about: Objects of a library by size, with owner and last use (QSYS2.OBJECT_STATISTICS).
-- needs: IBM i 7.1
SELECT OBJNAME, OBJTYPE, OBJSIZE, OBJOWNER, LAST_USED_TIMESTAMP, OBJTEXT
  FROM TABLE (QSYS2.OBJECT_STATISTICS('${library:QGPL}', '*ALL')) A
  ORDER BY OBJSIZE DESC
  FETCH FIRST ${rows:20} ROWS ONLY
