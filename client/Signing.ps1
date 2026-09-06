# PowerShell 7.2+ / .NET only. Does not install a certificate in any trust store.
function New-OperitSigningBundle {
    $rsa = [System.Security.Cryptography.RSA]::Create(3072)
    $cert = $null
    try {
        $request = [System.Security.Cryptography.X509Certificates.CertificateRequest]::new(
            'CN=Operit2 Personal Update, O=Unofficial Personal Build, C=CN', $rsa,
            [System.Security.Cryptography.HashAlgorithmName]::SHA256,
            [System.Security.Cryptography.RSASignaturePadding]::Pkcs1)
        $null = $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509BasicConstraintsExtension]::new($false, $false, 0, $true))
        $null = $request.CertificateExtensions.Add([System.Security.Cryptography.X509Certificates.X509KeyUsageExtension]::new(
            [System.Security.Cryptography.X509Certificates.X509KeyUsageFlags]::DigitalSignature, $true))
        $cert = $request.CreateSelfSigned([DateTimeOffset]::UtcNow.AddDays(-1), [DateTimeOffset]::UtcNow.AddYears(30))
        $password = [Convert]::ToHexString([System.Security.Cryptography.RandomNumberGenerator]::GetBytes(32))
        $pfx = $cert.Export([System.Security.Cryptography.X509Certificates.X509ContentType]::Pkcs12, $password)
        return (@{
            schema = 1
            pfx_base64 = [Convert]::ToBase64String($pfx)
            password = $password
            certificate_sha256 = $cert.GetCertHashString([System.Security.Cryptography.HashAlgorithmName]::SHA256).ToLowerInvariant()
        } | ConvertTo-Json -Compress)
    } finally {
        if ($cert) { $cert.Dispose() }
        $rsa.Dispose()
    }
}
