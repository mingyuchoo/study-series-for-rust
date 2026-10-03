package app.stillnote.data

internal fun sameBytes(a: ByteArray?, b: ByteArray?) =
    if (a == null || b == null) a == null && b == null else a.contentEquals(b)
