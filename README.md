# ssimoe
*S*erve *S*ingle *I*nput *M*ultiple *O*utput *E*vents
Duplicate a single input stream over multiple HTTP connections at a given server address

## Usage
To connect to a unix domain socket:
```
some program | ssimoe unix:/tmp/dirty.sock
```

To connect to a host:port pair:
```
some program | ssimoe localhost:8000
```
