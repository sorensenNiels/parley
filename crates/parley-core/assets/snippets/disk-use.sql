-- title: Disk use
-- tags: did you know, storage
-- about: Each disk unit's capacity, free space and percent used, fullest first (QSYS2.SYSDISKSTAT).
-- needs: IBM i 7.1
SELECT ASP_NUMBER, UNIT_NUMBER, UNIT_TYPE, UNIT_STORAGE_CAPACITY, UNIT_SPACE_AVAILABLE, PERCENT_USED
  FROM QSYS2.SYSDISKSTAT
  ORDER BY PERCENT_USED DESC
