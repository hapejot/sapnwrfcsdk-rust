# Compiling

## Linux

## Windows

On windows you need the LLVM libraries in order to generate the bindings for the SAP libraries.

The LLVM system can be downloaded prebuilt from github https://github.com/llvm/llvm-project/releases
The windows archive is compressed using the XZ-Tool which is needed first before it can be unpacked by TAR.
Both tools are part of the GIT-Shell distribution.

The only prerequisit for this project to compile ist the proper setting of the environment variable LIBCLANG_PATH,
which should point to the lib folder within the llvm distribution.

