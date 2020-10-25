Description

tar reads and writes headers in either the original TAR format from UNIX systems or the USTAR format defined by the POSIX 1003.1 standard.

A tar archive, in either format, consists of one or more blocks, which are used to represent member files. 
Each block is 512 bytes long; you can use the –b option with tar to indicate how many of these blocks are read or written (or both) at once.

Each member file consists of a header block, followed by zero or more blocks containing the file contents. 
The end of the archive is indicated by two blocks filled with binary zeros. Unused space in the header is left as binary zeros.

The header information in a block is stored in a printable ASCII form, so that tar archives are easily ported to different environments. 
If the contents of the files on the archive are all ASCII, the entire archive is ASCII.

Table 1 shows the UNIX format of the header block for a file:

Table 1. Archive file: UNIX-compatible formatField width 	Field Name 	Meaning

 * 100 	name 	Name of file
 * 8 	mode 	File mode
 * 8 	uid 	Owner user ID
 * 8 	gid 	Owner group ID
 * 12 	size 	Length of file in bytes
 * 12 	mtime 	Modify time of file
 * 8 	chksum 	Checksum for header
 * 1 	link 	Indicator for links
 * 100 	linkname 	Name of linked file

A directory is indicated by a trailing / (slash) in its name.
The link field is: 1 for a linked file, 2 for a symbolic link, 0 otherwise.

tar determines that the USTAR format is being used by the presence of the null-terminated string USTAR in the magic field. All fields before the magic field correspond to those of the UNIX format, except that typeflag replaces the link field.

Table 2. Archive file: USTAR formatField width 	Field name 	Meaning
 * 100 	name 	Name of file
 * 8 	mode 	File mode
 * 8 	uid 	Owner user ID
 * 8 	gid 	Owner group ID
 * 12 	size 	Length of file in bytes
 * 12 	mtime 	Modify time of file
 * 8 	chksum 	Checksum for header
 * 1 	typeflag 	Type of file
 * 100 	linkname 	Name of linked file
 * 6 	magic 	USTAR indicator
 * 2 	version 	USTAR version
 * 32 	uname 	Owner user name
 * 32 	gname 	Owner group name
 * 8 	devmajor 	Device major number
 * 8 	devminor 	Device minor number
 * 155 	prefix 	Prefix for file name

Description of the header files
In the headers:

The name field contains the name of the archived file. On USTAR format archives, the value of the prefix field, if non-null, is prefixed to the name field to allow names longer than 100 characters.
The magic, uname, and gname fields are null-terminated character strings
The name, linkname, and prefix fields are null-terminated unless the full field is used to store a name (that is, the last character is not null).
All other fields are zero-filled octal numbers, in ASCII. Trailing nulls are present for these numbers, except for the size, mtime, and version fields.
prefix is null unless the file name exceeds 100 characters.
The size field is zero if the header describes a link.
The chksum field is a checksum of all the bytes in the header, assuming that the chksum field itself is all blanks.
For USTAR, the typeflag field is a compatible extension of the link field of the older tar format. The following values are recognized:

Flag     File Type
 *  0 or null    Regular file
 * 1     Link to another file already archived
 * 2     Symbolic link
 * 3    Character special file
 * 4    Block special file (not supported)
 * 5    Directory
 * 6    FIFO special file
 * 7    Reserved
 * S    z/OS extended USTAR special header
 * T    z/OS extended USTAR special header summary (S and T are z/OS extensions. See z/OS-extended USTAR support for more information.)
 * A–Z    Available for custom usage

In USTAR format, the uname and gname fields contain the name of the owner and group of the file, respectively.

Compressed tar archives are equivalent to the corresponding archive being passed to a 14-bit compress command.
Related information