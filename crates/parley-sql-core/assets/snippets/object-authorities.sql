-- title: Who may use an object
-- tags: did you know, security
-- about: The users and groups with authority to an object, and what they may do (QSYS2.OBJECT_PRIVILEGES).
-- needs: IBM i 7.3
SELECT AUTHORIZATION_NAME, OBJECT_AUTHORITY, OBJECT_OPERATIONAL, OBJECT_MANAGEMENT, OBJECT_EXISTENCE,
       DATA_READ, DATA_ADD, DATA_UPDATE, DATA_DELETE
  FROM QSYS2.OBJECT_PRIVILEGES
  WHERE SYSTEM_OBJECT_SCHEMA = '${library:QGPL}' AND SYSTEM_OBJECT_NAME = '${object:QCLSRC}'
  ORDER BY AUTHORIZATION_NAME
