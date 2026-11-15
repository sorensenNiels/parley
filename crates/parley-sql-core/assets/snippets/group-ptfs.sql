-- title: Group PTFs installed
-- tags: did you know, system
-- about: The PTF groups on this system, their level and status (QSYS2.GROUP_PTF_INFO).
-- needs: IBM i 7.2
SELECT PTF_GROUP_NAME, PTF_GROUP_DESCRIPTION, PTF_GROUP_LEVEL, PTF_GROUP_STATUS
  FROM QSYS2.GROUP_PTF_INFO
  ORDER BY PTF_GROUP_NAME, PTF_GROUP_LEVEL DESC
