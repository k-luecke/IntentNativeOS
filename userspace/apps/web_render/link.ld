ENTRY(_start)

SECTIONS {
    . = 0x00400000;

    .text : {
        *(.text._start)
        *(.text .text.*)
    }

    .rodata : {
        *(.rodata .rodata.*)
    }

    . = 0x00480000;

    .got : {
        *(.got .got.*)
    }

    .data : {
        *(.data .data.*)
    }

    .bss : {
        *(.bss .bss.*)
    }
}
