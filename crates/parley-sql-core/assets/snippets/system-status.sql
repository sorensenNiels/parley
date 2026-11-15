-- title: System status
-- tags: did you know, system
-- about: Processor, memory, storage and job counts, as WRKSYSSTS shows them (QSYS2.SYSTEM_STATUS_INFO).
-- needs: IBM i 7.2
SELECT * FROM QSYS2.SYSTEM_STATUS_INFO
