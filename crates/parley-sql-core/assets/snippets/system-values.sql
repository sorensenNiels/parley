-- title: System values
-- tags: did you know, system
-- about: Every system value and its current setting, as WRKSYSVAL shows them (QSYS2.SYSTEM_VALUE_INFO).
-- needs: IBM i 7.1
SELECT SYSTEM_VALUE_NAME, CURRENT_NUMERIC_VALUE, CURRENT_CHARACTER_VALUE
  FROM QSYS2.SYSTEM_VALUE_INFO
  ORDER BY SYSTEM_VALUE_NAME
