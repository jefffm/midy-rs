MEMORY
{
  /* Flash memory begins at 0x80000000 and has a size of 64kB*/
  FLASH : ORIGIN = 0x08000000, LENGTH = 64K

  /* On the discovery, we have 128k*/
  /* FLASH : ORIGIN = 0x08000000, LENGTH = 128K */

  /* RAM begins at 0x20000000 and has a size of 20kB*/
  RAM : ORIGIN = 0x20000000, LENGTH = 20K
}
