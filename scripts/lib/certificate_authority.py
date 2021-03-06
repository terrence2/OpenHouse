import os
from . import call, change_directory, make_directory, replace_file

CONFIG_TEMPLATE = """
[ ca ]
# `man ca`
default_ca = CA_default

[ CA_default ]
# Directory and file locations.
dir               = {cert_dir}
certs             = $dir/certs
crl_dir           = $dir/crl
new_certs_dir     = $dir/newcerts
database          = $dir/index.txt
serial            = $dir/serial
RANDFILE          = $dir/private/.rand

# The root key and root certificate.
private_key       = $dir/private/{name}.key.pem
certificate       = $dir/certs/{name}.cert.pem

# For certificate revocation lists.
crlnumber         = $dir/crlnumber
crl               = $dir/crl/{name}.crl.pem
crl_extensions    = crl_ext
default_crl_days  = 30

# SHA-1 is deprecated, so use SHA-2 instead.
default_md        = sha256

name_opt          = ca_default
cert_opt          = ca_default
default_days      = 375
preserve          = no
policy            = {policy}

[ policy_strict ]
# The root CA should only sign intermediate certificates that match.
# See the POLICY FORMAT section of `man ca`.
countryName             = match
stateOrProvinceName     = match
organizationName        = match
organizationalUnitName  = optional
commonName              = supplied
emailAddress            = optional

[ policy_loose ]
# Allow the intermediate CA to sign a more diverse range of certificates.
# See the POLICY FORMAT section of the `ca` man page.
countryName             = optional
stateOrProvinceName     = optional
localityName            = optional
organizationName        = optional
organizationalUnitName  = optional
commonName              = supplied
emailAddress            = optional

[ req ]
# Options for the `req` tool (`man req`).
default_bits        = 2048
distinguished_name  = req_distinguished_name
string_mask         = utf8only

# SHA-1 is deprecated, so use SHA-2 instead.
default_md          = sha256

# Extension to add when the -x509 option is used.
x509_extensions     = v3_ca

[ req_distinguished_name ]
# See <https://en.wikipedia.org/wiki/Certificate_signing_request>.
countryName                     = Country Name (2 letter code)
stateOrProvinceName             = State or Province Name
localityName                    = Locality Name
0.organizationName              = Organization Name
organizationalUnitName          = Organizational Unit Name
commonName                      = Common Name
emailAddress                    = Email Address

[ v3_ca ]
# Extensions for a typical CA (`man x509v3_config`).
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer
basicConstraints = critical, CA:true
keyUsage = critical, digitalSignature, cRLSign, keyCertSign

[ v3_intermediate_ca ]
# Extensions for a typical intermediate CA (`man x509v3_config`).
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid:always,issuer
basicConstraints = critical, CA:true, pathlen:0
keyUsage = critical, digitalSignature, cRLSign, keyCertSign

[ usr_cert ]
# Extensions for client certificates (`man x509v3_config`).
basicConstraints = CA:FALSE
nsCertType = client, email
nsComment = "OpenSSL Generated Client Certificate"
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer
keyUsage = critical, nonRepudiation, digitalSignature, keyEncipherment
extendedKeyUsage = clientAuth, emailProtection

[ server_cert ]
# Extensions for server certificates (`man x509v3_config`).
basicConstraints = CA:FALSE
nsCertType = server
nsComment = "OpenSSL Generated Server Certificate"
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer:always
keyUsage = critical, digitalSignature, keyEncipherment
extendedKeyUsage = serverAuth

[ crl_ext ]
# Extension for CRLs (`man x509v3_config`).
authorityKeyIdentifier=keyid:always

[ ocsp ]
# Extension for OCSP signing certificates (`man ocsp`).
basicConstraints = CA:FALSE
subjectKeyIdentifier = hash
authorityKeyIdentifier = keyid,issuer
keyUsage = critical, digitalSignature
extendedKeyUsage = critical, OCSPSigning
"""


def _make_ca_directory_structure():
    make_directory("CA")
    with change_directory("CA"):
        _make_sub_ca_directory_structure("root")
        _write_ca_configuration(cert_dir="root", name="ca", policy="policy_strict")

        _make_sub_ca_directory_structure("intermediate")
        _write_ca_configuration(cert_dir="intermediate", name="intermediate", policy="policy_loose")


def _make_sub_ca_directory_structure(root: str):
    assert "/" not in root
    make_directory(root)
    with change_directory(root):
        make_directory("certs")
        make_directory("csr", 0o700)
        make_directory("crl", 0o700)
        make_directory("newcerts", 0o700)
        make_directory("private", 0o700)


def _write_ca_configuration(**params):
    with change_directory(params['cert_dir']):
        config = CONFIG_TEMPLATE.format(**params)
        replace_file("openssl.cnf", config)
        replace_file("index.txt", "")
        replace_file("serial", "1000")
        replace_file("crlnumber", "1000")


def make_certificate_authority(key_size: int, expire: int, key_security: str, x509_security: str):
    _make_ca_directory_structure()

    params = {
        'key_size': key_size,
        'expire': expire,
        'key_security': key_security,
        'x509_security': x509_security
    }

    with change_directory("CA"):
        # Generate the root CA key pair.
        #
        # You should burn this to a CD after this is done and save it in a vault in
        # case you need to regenerate the following later. Or just regenerate the
        # entire chain, since it's all under your own control anyway.
        with change_directory("root"):
            # Generate the CA signing key.
            call("openssl genrsa {key_security} -out private/ca.key.pem {key_size}", **params)
            os.chmod("private/ca.key.pem", 0o400)

            # Create the CA root certificate.
            call("""openssl req -config openssl.cnf -key private/ca.key.pem
                                    -new -x509 {x509_security} -days {expire}
                                    -subj /C=US/ST=CA/L=SB/O=Me/OU=OpenHouse/CN=certificate_authority/
                                    -extensions v3_ca
                                    -out certs/ca.cert.pem""", **params)
            os.chmod("certs/ca.cert.pem", 0o444)

        # Make an intermediate signing CA key pair.
        # This is the in-use signing key for daily use.
        with change_directory("intermediate"):
            # Generate the intermediate signing key.
            call("openssl genrsa {key_security} -out private/intermediate.key.pem {key_size}", **params)
            os.chmod("private/intermediate.key.pem", 0o400)

            # Create a certificate signing request for the intermediate.
            call("""openssl req -config openssl.cnf -new {x509_security}
                                    -subj /C=US/ST=CA/L=SB/O=Me/OU=OpenHouse/CN=intermediate_authority/
                                    -key private/intermediate.key.pem
                                    -out csr/intermediate.csr.pem""", **params)

        # Sign the intermediate CA with the root CA, generating the certificate.
        call("""openssl ca -config root/openssl.cnf
                               -extensions v3_intermediate_ca -batch
                               -days {expire} -notext -md sha256
                               -in intermediate/csr/intermediate.csr.pem
                               -out intermediate/certs/intermediate.cert.pem""", **params)
        os.chmod("intermediate/certs/intermediate.cert.pem", 0o444)

        # Verify.
        call("openssl verify -CAfile root/certs/ca.cert.pem intermediate/certs/intermediate.cert.pem")

        # Make a conjoined chain certificate.
        with open("root/certs/ca.cert.pem", "r") as fp_root:
            with open("intermediate/certs/intermediate.cert.pem", "r") as fp_intermediate:
                with open("intermediate/certs/chain.cert.pem", "w") as fp_chain:
                    fp_chain.write(fp_root.read())
                    fp_chain.write(fp_intermediate.read())


def _make_certificate(name: str, extension: str, key_size: int, expire: int,
                      key_security: str, x509_security: str):
    assert os.path.isdir('CA')

    params = {
        'name': name,
        'extension': extension,
        'key_size': key_size,
        'expire': expire,
        'key_security': key_security,
        'x509_security': x509_security
    }
    with change_directory("CA"):
        with change_directory("intermediate"):
            call("openssl genrsa {key_security} -out private/{name}.key.pem {key_size}", **params)
            os.chmod("private/{name}.key.pem".format(**params), 0o400)
            call("""openssl req -config openssl.cnf -new {x509_security}
                                        -subj /C=US/ST=CA/L=SB/O=Me/OU=OpenHouse/CN={name}/
                                        -key private/{name}.key.pem
                                        -out csr/{name}.csr.pem""", **params)
        call("""openssl ca -config intermediate/openssl.cnf
                               -extensions {extension} -batch
                               -days {expire} -notext -md sha256
                               -in intermediate/csr/{name}.csr.pem
                               -out intermediate/certs/{name}.cert.pem""",
             **params)
        os.chmod("intermediate/certs/{name}.cert.pem".format(**params), 0o444)


def make_server_certificate(name: str, key_size: int, expire: int, key_security: str, x509_security: str):
    _make_certificate(name, 'server_cert', key_size=key_size, expire=expire,
                      key_security=key_security, x509_security=x509_security)


def make_client_certificate(name: str, key_size: int, expire: int, key_security: str, x509_security: str):
    _make_certificate(name, 'usr_cert', key_size=key_size, expire=expire,
                      key_security=key_security, x509_security=x509_security)

