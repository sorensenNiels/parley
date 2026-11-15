-- title: The largest IFS files
-- tags: did you know, storage, ifs
-- about: The largest stream files under a folder, with owner and last use (QSYS2.IFS_OBJECT_STATISTICS). Can take a while on a large folder.
-- needs: IBM i 7.4
SELECT PATH_NAME, DATA_SIZE, OBJECT_OWNER, LAST_USED_TIMESTAMP
  FROM TABLE (QSYS2.IFS_OBJECT_STATISTICS(
         START_PATH_NAME => '${folder:/home}', SUBTREE_DIRECTORIES => 'YES')) A
  WHERE OBJECT_TYPE = '*STMF'
  ORDER BY DATA_SIZE DESC
  FETCH FIRST ${rows:20} ROWS ONLY
