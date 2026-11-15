-- title: Data areas in a library
-- tags: did you know, objects
-- about: The data areas of a library with their type, length and value (QSYS2.DATA_AREA_INFO).
-- needs: IBM i 7.3
SELECT DATA_AREA_NAME, DATA_AREA_TYPE, LENGTH, DECIMAL_POSITIONS, DATA_AREA_VALUE
  FROM QSYS2.DATA_AREA_INFO
  WHERE DATA_AREA_LIBRARY = '${library:QGPL}'
  ORDER BY DATA_AREA_NAME
