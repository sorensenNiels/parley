-- title: Who has an object locked
-- tags: did you know, work management
-- about: The jobs holding locks on an object, and the lock state (QSYS2.OBJECT_LOCK_INFO).
-- needs: IBM i 7.2
SELECT JOB_NAME, LOCK_STATE, LOCK_STATUS, LOCK_SCOPE, OBJECT_TYPE
  FROM QSYS2.OBJECT_LOCK_INFO
  WHERE OBJECT_SCHEMA = UPPER('${library}') AND OBJECT_NAME = UPPER('${object}')
