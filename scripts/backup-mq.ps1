# MQ Backup Script — dijalankan Task Scheduler 02:00 harian
# Retensi 14 hari. Output: C:\mq-data\backups\mq\mq-YYYYMMDD.sql.gz

$ErrorActionPreference = "Stop"

$BackupDir = "C:\mq-data\backups\mq"
$MysqlDump = "C:\laragon\bin\mysql\mysql-8.0.30-winx64\bin\mysqldump.exe"
$Date = Get-Date -Format "yyyyMMdd"
$OutFile = "$BackupDir\mq-$Date.sql.gz"

# Password dari file (tidak hardcoded)
$PassFile = "$env:USERPROFILE\.mq_dbpass.txt"
if (-not (Test-Path $PassFile)) {
    Write-Error "Password file tidak ditemukan: $PassFile"
    exit 1
}
$DbPass = (Get-Content $PassFile -Raw).Trim()

# Pastikan folder ada
New-Item -ItemType Directory -Path $BackupDir -Force | Out-Null

# Dump (pipe via cmd gzip alternatif: 7za atau .NET GZipStream)
$TempSql = "$env:TEMP\mq-backup-temp.sql"
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Dumping mq_dev..."

$proc = Start-Process -FilePath $MysqlDump -ArgumentList @(
    "--single-transaction", "--set-gtid-purged=OFF",
    "-u", "mq_app", "-p$DbPass", "-h", "127.0.0.1", "mq_dev"
) -RedirectStandardOutput $TempSql -RedirectStandardError "$env:TEMP\mq-dump-err.txt" -NoNewWindow -Wait -PassThru

if ($proc.ExitCode -ne 0) {
    $err = Get-Content "$env:TEMP\mq-dump-err.txt" -Raw
    Write-Error "mysqldump gagal (exit $($proc.ExitCode)): $err"
    exit 1
}

# Compress via .NET GZipStream (tidak butuh gzip.exe)
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Compressing..."
$inputStream = [System.IO.File]::OpenRead($TempSql)
$outputStream = [System.IO.File]::Create($OutFile)
$gzipStream = New-Object System.IO.Compression.GZipStream($outputStream, [System.IO.Compression.CompressionLevel]::Optimal)
$inputStream.CopyTo($gzipStream)
$gzipStream.Close(); $outputStream.Close(); $inputStream.Close()
Remove-Item $TempSql -Force

$Size = (Get-Item $OutFile).Length / 1KB
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] OK: $([math]::Round($Size, 1)) KB -> $OutFile"

# Retensi 14 hari
$Cutoff = (Get-Date).AddDays(-14)
Get-ChildItem $BackupDir -Filter "mq-*.sql.gz" | Where-Object { $_.LastWriteTime -lt $Cutoff } | Remove-Item -Force
Write-Host "[$(Get-Date -Format 'HH:mm:ss')] Retensi: file > 14 hari dihapus"

# Cek SeaweedFS data
$SeaweedData = "C:\mq-data\seaweedfs"
if (Test-Path $SeaweedData) {
    $DataSize = (Get-ChildItem $SeaweedData -Recurse -ErrorAction SilentlyContinue | Measure-Object -Property Length -Sum).Sum / 1MB
    Write-Host "[$(Get-Date -Format 'HH:mm:ss')] SeaweedFS data: $([math]::Round($DataSize, 1)) MB"
}
